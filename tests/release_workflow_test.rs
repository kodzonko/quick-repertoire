use std::fs;
use std::path::Path;
#[cfg(unix)]
use std::process::{Command, Output};

use regex::Regex;

const RELEASE_WORKFLOW_PATH: &str = ".github/workflows/release.yml";
const LINT_TEST_WORKFLOW_PATH: &str = ".github/workflows/lint-test.yml";
#[cfg(unix)]
const RELEASE_TAG_CHECK_PATH: &str = ".github/scripts/check-release-tag.sh";
const CHANGELOG_PATH: &str = "CHANGELOG.md";
const VERIFIED_TAG_REF: &str = "ref: refs/tags/${{ needs.verify-tag.outputs.tag }}";

/// Reads a repository file with line endings normalized to `\n`, so assertions
/// hold on Windows checkouts that convert files to CRLF.
fn read_repo_file(path: &str) -> String {
    fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
        .replace("\r\n", "\n")
}

/// Returns the `version` declared in the `[package]` section of a Cargo manifest.
fn package_version(manifest: &str) -> String {
    Regex::new(r#"(?ms)^\[package\]$.*?^version = "([^"]+)""#)
        .unwrap()
        .captures(manifest)
        .and_then(|captures| captures.get(1))
        .map(|capture| capture.as_str().to_string())
        .expect("expected Cargo.toml to declare a [package] version")
}

/// Returns the lines of a top-level workflow job, from its `  <job>:` line up to the next job.
fn job_block<'a>(workflow: &'a str, job: &str) -> &'a str {
    let start = workflow
        .find(&format!("\n  {job}:\n"))
        .unwrap_or_else(|| panic!("expected release.yml to define the `{job}` job"))
        + 1;
    let rest = &workflow[start..];
    let end = Regex::new(r"(?m)^  \S")
        .unwrap()
        .find_iter(rest)
        .nth(1)
        .map_or(rest.len(), |next_job| next_job.start());

    &rest[..end]
}

#[cfg(unix)]
fn run_release_tag_check(tag: &str) -> Output {
    Command::new("bash")
        .arg(RELEASE_TAG_CHECK_PATH)
        .arg(tag)
        .output()
        .expect("failed to run the release tag check")
}

#[test]
fn release_workflow_starts_when_release_is_published() {
    let workflow = read_repo_file(RELEASE_WORKFLOW_PATH);

    assert!(workflow.contains("on:\n  release:\n    types: [published]"));
    assert!(!workflow.contains("  push:\n"));
}

fn package_binary_name(manifest: &str) -> String {
    let binary_name_pattern = Regex::new(r#"(?ms)\[\[bin\]\].*?^name = "([^"]+)""#).unwrap();

    binary_name_pattern
        .captures(manifest)
        .and_then(|captures| captures.get(1))
        .map(|capture| capture.as_str().to_string())
        .expect("expected Cargo.toml to declare a [[bin]] name")
}

#[test]
fn release_workflow_builds_and_packages_the_declared_binary() {
    let manifest = read_repo_file("Cargo.toml");
    let workflow = read_repo_file(RELEASE_WORKFLOW_PATH);
    let package_binary_name = package_binary_name(&manifest);

    let workflow_binary_name = Regex::new(r#"(?m)^\s*BINARY_NAME:\s+([A-Za-z0-9_-]+)\s*$"#)
        .unwrap()
        .captures(&workflow)
        .and_then(|captures| captures.get(1))
        .map(|capture| capture.as_str().to_string())
        .expect("expected release.yml to declare BINARY_NAME");

    assert_eq!(
        workflow_binary_name, package_binary_name,
        "release workflow builds `{workflow_binary_name}`, but Cargo.toml declares `{package_binary_name}`"
    );

    let binary_paths = Regex::new(r#"(?m)^\s*binary_path:\s+(\S+)\s*$"#)
        .unwrap()
        .captures_iter(&workflow)
        .map(|captures| captures[1].to_string())
        .collect::<Vec<_>>();

    assert!(!binary_paths.is_empty(), "expected release.yml to declare native binary paths");

    for binary_path in binary_paths {
        let expected_file_name = if binary_path.ends_with(".exe") {
            format!("{package_binary_name}.exe")
        } else {
            package_binary_name.clone()
        };

        assert_eq!(
            Path::new(&binary_path).file_name().and_then(|name| name.to_str()),
            Some(expected_file_name.as_str()),
            "release workflow packages `{binary_path}`, but Cargo.toml declares `{package_binary_name}`"
        );
    }
}

#[test]
fn rust_ci_workflow_uses_stable_toolchain() {
    let workflow = read_repo_file(LINT_TEST_WORKFLOW_PATH);

    let tools = read_repo_file("mise.toml");

    assert!(
        workflow.contains("uses: jdx/mise-action@v5"),
        "expected lint-test.yml to install the toolchain through mise"
    );
    assert!(tools.contains(r#"version = "stable""#));
    assert!(
        !workflow.contains("uses: dtolnay/rust-toolchain@nightly"),
        "lint-test.yml should not install the nightly Rust toolchain"
    );
    assert!(
        !workflow.contains("cargo llvm-cov --locked --branch"),
        "lint-test.yml should not request branch coverage when running on stable"
    );
}

#[test]
fn release_workflow_uploads_assets_to_existing_release() {
    let workflow = read_repo_file(RELEASE_WORKFLOW_PATH);

    assert!(
        workflow.contains(
            r#"gh release upload "${RELEASE_TAG}" "${assets[@]}" --repo "${GITHUB_REPOSITORY}" --clobber"#
        ),
        "release.yml should scope `gh release upload` to GITHUB_REPOSITORY"
    );
    assert!(!workflow.contains("gh release create"));
    assert!(!workflow.contains("release_target"));
}

#[test]
fn release_workflow_verifies_the_tag_before_building_or_publishing() {
    let workflow = read_repo_file(RELEASE_WORKFLOW_PATH);

    assert!(
        job_block(&workflow, "verify-tag")
            .contains(r#"bash .github/scripts/check-release-tag.sh "${RELEASE_TAG}""#),
        "verify-tag should run the release tag check"
    );

    let needs_verify_tag = Regex::new(r"(?m)^\s+(needs: verify-tag|- verify-tag)$").unwrap();

    for job in ["build-linux", "build-native", "publish-release"] {
        assert!(
            needs_verify_tag.is_match(job_block(&workflow, job)),
            "`{job}` should wait for verify-tag, so a mismatched tag never ships binaries"
        );
    }
}

#[test]
fn release_workflow_builds_and_publishes_the_verified_tag() {
    let workflow = read_repo_file(RELEASE_WORKFLOW_PATH);

    assert!(
        !workflow.contains("${{ github.ref }}"),
        "manual runs see the default branch as github.ref: builds would use it instead of the \
         release tag, and runs for different tags would cancel each other"
    );

    for job in ["build-linux", "build-native"] {
        assert!(
            job_block(&workflow, job).contains(VERIFIED_TAG_REF),
            "`{job}` should check out the verified release tag"
        );
    }

    assert!(
        job_block(&workflow, "publish-release")
            .contains("RELEASE_TAG: ${{ needs.verify-tag.outputs.tag }}"),
        "publish-release should upload to the verified release tag"
    );
}

#[cfg(unix)]
#[test]
fn release_tag_check_accepts_v_prefixed_package_version() {
    let version = package_version(&read_repo_file("Cargo.toml"));

    let output = run_release_tag_check(&format!("v{version}"));

    assert!(
        output.status.success(),
        "expected `v{version}` to pass the release tag check: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
#[test]
fn release_tag_check_rejects_tags_that_do_not_match_package_version() {
    let version = package_version(&read_repo_file("Cargo.toml"));
    let rejected_tags = [
        version.clone(),
        format!("V{version}"),
        format!("v{version}.1"),
        format!("release-{version}"),
        "v0.0.0".to_string(),
        String::new(),
    ];

    for tag in rejected_tags {
        let output = run_release_tag_check(&tag);

        assert!(
            !output.status.success(),
            "expected tag `{tag}` to be rejected for Cargo.toml version {version}"
        );
    }
}

#[test]
fn changelog_latest_release_matches_package_version() {
    let version = package_version(&read_repo_file("Cargo.toml"));
    let changelog = read_repo_file(CHANGELOG_PATH);

    let latest_release = Regex::new(r"(?m)^## \[(\d+\.\d+\.\d+[^\]]*)\]")
        .unwrap()
        .captures(&changelog)
        .map(|captures| captures[1].to_string())
        .expect("expected CHANGELOG.md to contain a released version section");

    assert_eq!(
        latest_release, version,
        "Cargo.toml version {version} differs from the newest CHANGELOG.md release {latest_release}; \
         bump both together when cutting a release (`make changelog VERSION=...`)"
    );
}

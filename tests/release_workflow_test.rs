use std::fs;
use std::path::Path;

use regex::Regex;

const RELEASE_WORKFLOW_PATH: &str = ".github/workflows/release.yml";
const LINT_TEST_WORKFLOW_PATH: &str = ".github/workflows/lint-test.yml";
const CLAUDE_WORKFLOW_PATH: &str = ".github/workflows/claude.yml";
const CLAUDE_REVIEW_WORKFLOW_PATH: &str = ".github/workflows/claude-code-review.yml";
const CLAUDE_ALLOWED_LOGIN: &str = "kodzonko";

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
    let manifest = fs::read_to_string("Cargo.toml").unwrap();
    let workflow = fs::read_to_string(RELEASE_WORKFLOW_PATH).unwrap();
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
    let workflow = fs::read_to_string(LINT_TEST_WORKFLOW_PATH).unwrap();

    assert!(
        workflow.contains("uses: dtolnay/rust-toolchain@stable"),
        "expected lint-test.yml to install the stable Rust toolchain"
    );
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
fn release_workflow_publishes_without_git_checkout_context() {
    let workflow = fs::read_to_string(RELEASE_WORKFLOW_PATH).unwrap();

    assert!(
        workflow.contains(r#"gh release view "${RELEASE_TAG}" --repo "${GITHUB_REPOSITORY}""#),
        "release.yml should scope `gh release view` to GITHUB_REPOSITORY"
    );
    assert!(
        workflow.contains(
            r#"gh release upload "${RELEASE_TAG}" "${assets[@]}" --repo "${GITHUB_REPOSITORY}" --clobber"#
        ),
        "release.yml should scope `gh release upload` to GITHUB_REPOSITORY"
    );
    assert!(
        workflow.contains(
            r#"gh release create "${RELEASE_TAG}" "${assets[@]}" --repo "${GITHUB_REPOSITORY}" --target "${RELEASE_TARGET}" --generate-notes"#
        ),
        "release.yml should scope `gh release create` to GITHUB_REPOSITORY"
    );
}

#[test]
fn claude_workflow_only_allows_the_configured_login_to_trigger_mentions() {
    let workflow = fs::read_to_string(CLAUDE_WORKFLOW_PATH).unwrap();
    let allowed_login_guard = format!("github.event.sender.login == '{CLAUDE_ALLOWED_LOGIN}'");

    assert!(
        workflow.contains(&allowed_login_guard),
        "claude.yml should restrict Claude mentions to `{CLAUDE_ALLOWED_LOGIN}`"
    );
}

#[test]
fn claude_review_workflow_only_runs_for_the_configured_pr_author() {
    let workflow = fs::read_to_string(CLAUDE_REVIEW_WORKFLOW_PATH).unwrap();
    let allowed_login_guard =
        format!("github.event.pull_request.user.login == '{CLAUDE_ALLOWED_LOGIN}'");

    assert!(
        workflow.contains(&allowed_login_guard),
        "claude-code-review.yml should restrict PR reviews to `{CLAUDE_ALLOWED_LOGIN}`"
    );
}

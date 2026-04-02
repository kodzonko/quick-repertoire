use std::fs;
use std::path::Path;

use regex::Regex;

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
    let workflow = fs::read_to_string(".github/workflows/build-binaries.yml").unwrap();
    let package_binary_name = package_binary_name(&manifest);

    let workflow_binary_name = Regex::new(r#"(?m)^\s*BINARY_NAME:\s+([A-Za-z0-9_-]+)\s*$"#)
        .unwrap()
        .captures(&workflow)
        .and_then(|captures| captures.get(1))
        .map(|capture| capture.as_str().to_string())
        .expect("expected build-binaries.yml to declare BINARY_NAME");

    assert_eq!(
        workflow_binary_name, package_binary_name,
        "release workflow builds `{workflow_binary_name}`, but Cargo.toml declares `{package_binary_name}`"
    );

    let binary_paths = Regex::new(r#"(?m)^\s*binary_path:\s+(\S+)\s*$"#)
        .unwrap()
        .captures_iter(&workflow)
        .map(|captures| captures[1].to_string())
        .collect::<Vec<_>>();

    assert!(!binary_paths.is_empty(), "expected build-binaries.yml to declare native binary paths");

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

use pretty_assertions::assert_eq;
use std::fs;
use std::process::Command;
use tempfile::tempdir;

// TODO: test ignored because nix builds failing
#[test]
#[ignore]
fn test_scan_layouts_integration() {
    // Create temp dir mimicking zellij session structure
    let temp = tempdir().unwrap();
    let session_dir = temp.path().join("session_info/my_session");
    fs::create_dir_all(&session_dir).unwrap();

    let session_file = session_dir.join("session-layout.kdl");
    fs::copy(
        "tests/fixtures/mediactl_input/session-layout.kdl",
        &session_file,
    )
    .unwrap();

    // Build first to ensure binary is up to date
    let build = Command::new("cargo")
        .args(["build"])
        .output()
        .expect("Failed to build");
    assert!(build.status.success(), "Build failed: {:?}", build);

    // Run the CLI
    let output = Command::new("cargo")
        .args([
            "run",
            "--quiet",
            "--",
            "scan-layouts",
            "--quiet",
            temp.path().to_str().unwrap(),
        ])
        .output()
        .expect("Failed to run command");

    assert!(
        output.status.success(),
        "Command failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Compare result to expected
    let result = fs::read_to_string(&session_file).unwrap();
    let expected =
        fs::read_to_string("tests/fixtures/mediactl_expected/session-layout.kdl").unwrap();

    assert_eq!(result.trim(), expected.trim());
}

//! Exercise the real packaging helper in an isolated directory. Fixture payloads
//! test assembly only; cross-platform executable correctness is verified by CI.
#![cfg(unix)]
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_owned()
}
fn package(version: &str, arch: &str, binaries: bool) -> (tempfile::TempDir, Output) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("packaging/linux/debian")).unwrap();
    for name in ["debian/control", "shellumi.desktop"] {
        std::fs::copy(
            root().join("packaging/linux").join(name),
            dir.path().join("packaging/linux").join(name),
        )
        .unwrap();
    }
    if binaries {
        std::fs::write(dir.path().join("app"), b"packaging-fixture-app").unwrap();
        std::fs::write(dir.path().join("cli"), b"packaging-fixture-cli").unwrap();
    }
    let output = Command::new("bash")
        .arg(root().join("packaging/linux/build-deb.sh"))
        .args([arch, "app", "cli"])
        .current_dir(dir.path())
        .env("SHELLUMI_VERSION", version)
        .output()
        .unwrap();
    (dir, output)
}
#[test]
fn prerelease_debian_version_is_literal_and_both_binaries_are_packaged() {
    let (dir, output) = package("0.0.2-alpha", "x86_64", true);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let folder = dir.path().join("dist/shellumi_0.0.2-alpha_amd64");
    let control = std::fs::read_to_string(folder.join("DEBIAN/control")).unwrap();
    assert!(control.lines().any(|line| line == "Version: 0.0.2~alpha"));
    assert!(control.lines().any(|line| line == "Architecture: amd64"));
    assert!(!control.contains(&dir.path().display().to_string()));
    assert_eq!(
        std::fs::read(folder.join("usr/bin/shellumi-app")).unwrap(),
        b"packaging-fixture-app"
    );
    assert_eq!(
        std::fs::read(folder.join("usr/bin/shellumi-cli")).unwrap(),
        b"packaging-fixture-cli"
    );
}
#[test]
fn invalid_packaging_inputs_fail_before_any_output_cleanup() {
    for (version, arch, binaries) in [
        ("../../escape", "amd64", true),
        ("0.0.2-alpha", "../escape", true),
        ("0.0.2-alpha", "amd64", false),
    ] {
        let (dir, output) = package(version, arch, binaries);
        assert!(!output.status.success());
        assert!(!dir.path().join("dist").exists());
    }
}
#[test]
fn stable_arm_package_keeps_version_and_normalizes_architecture() {
    let (dir, output) = package("1.2.3", "aarch64", true);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let control =
        std::fs::read_to_string(dir.path().join("dist/shellumi_1.2.3_arm64/DEBIAN/control"))
            .unwrap();
    assert!(control.lines().any(|line| line == "Version: 1.2.3"));
    assert!(control.lines().any(|line| line == "Architecture: arm64"));
}

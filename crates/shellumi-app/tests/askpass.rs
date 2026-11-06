use std::process::{Command, Output};

fn helper(prompt: &str, kind: &str, secret: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_shellumi-app"))
        .arg(prompt)
        .env("SHELLUMI_ASKPASS_MODE", "1")
        .env("SHELLUMI_ASKPASS_TARGET", "deploy@example.test")
        .env("SHELLUMI_ASKPASS_KEY", "/keys/test key")
        .env(
            "SHELLUMI_ASKPASS_PASSWORD_REF",
            "env://SHELLUMI_TEST_SECRET",
        )
        .env(
            "SHELLUMI_ASKPASS_PASSPHRASE_REF",
            "env://SHELLUMI_TEST_SECRET",
        )
        .env("SHELLUMI_TEST_SECRET", secret)
        .env("SSH_ASKPASS_PROMPT", kind)
        .env_remove("SHELLUMI_CREDENTIAL_VAULT_PASSPHRASE")
        .output()
        .unwrap()
}
#[test]
fn helper_supplies_only_matching_password_and_passphrase() {
    for prompt in [
        "deploy@example.test's password: ",
        "Enter passphrase for key '/keys/test key': ",
    ] {
        let output = helper(prompt, "", "test-fixture-credential");
        assert!(output.status.success());
        assert_eq!(output.stdout, b"test-fixture-credential\n");
        assert!(output.stderr.is_empty());
    }
}
#[test]
fn helper_fails_closed_without_output() {
    for (prompt, kind, secret) in [
        ("deploy@example.test's password:", "confirm", "fixture"),
        (
            "Are you sure you want to continue connecting?",
            "",
            "fixture",
        ),
        ("Verification code:", "", "fixture"),
        ("Password:", "", "fixture"),
        ("root@example.test's password:", "", "fixture"),
        ("Enter passphrase for key '/other':", "", "fixture"),
        ("deploy@example.test's password:", "", "bad\ncredential"),
    ] {
        let output = helper(prompt, kind, secret);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }
}

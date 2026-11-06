//! Opt-in real OpenSSH acceptance; never touches the user's SSH configuration.
#![cfg(target_os = "macos")]

use shellumi_core::{
    AuthMethod, HostGroup, NewIdentity, SftpClient, SftpConnectOptions, SshDestination,
};
use shellumi_storage::HostStore;
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

struct Server {
    child: Child,
}
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn keygen(path: &Path, passphrase: &str) {
    let output = Command::new("/usr/bin/ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", passphrase, "-f"])
        .arg(path)
        .output()
        .unwrap();
    assert!(output.status.success(), "Test key generation failed");
}
fn text(path: &Path) -> &str {
    path.to_str().unwrap()
}

#[test]
#[ignore = "requires macOS OpenSSH sshd and permission to bind a loopback socket"]
fn saved_identity_sftp_roundtrip_reconnect_and_host_trust() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let key = root.join("encrypted key with spaces");
    let host_key = root.join("host_key");
    let wrong_key = root.join("wrong_host_key");
    // Disposable fixture material only. No real account password or user keys are used.
    const PASSPHRASE: &str = "shellumi-disposable-test-passphrase";
    keygen(&key, PASSPHRASE);
    keygen(&host_key, "");
    keygen(&wrong_key, "");
    let authorized = root.join("authorized_keys");
    std::fs::copy(key.with_extension("pub"), &authorized).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let config = root.join("sshd_config");
    std::fs::write(&config, format!(
        "Port {port}\nListenAddress 127.0.0.1\nHostKey {}\nPidFile {}\nAuthorizedKeysFile {}\nPasswordAuthentication no\nKbdInteractiveAuthentication no\nPubkeyAuthentication yes\nUsePAM no\nStrictModes no\nUseDNS no\nAllowAgentForwarding no\nAllowTcpForwarding no\nX11Forwarding no\nSubsystem sftp internal-sftp\nForceCommand internal-sftp\n",
        host_key.display(), root.join("sshd.pid").display(), authorized.display()
    )).unwrap();
    let mut server = Server {
        child: Command::new("/usr/sbin/sshd")
            .args(["-D", "-f"])
            .arg(&config)
            .arg("-E")
            .arg(root.join("sshd.log"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    };
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        assert!(
            server.child.try_wait().unwrap().is_none(),
            "sshd exited: {}",
            std::fs::read_to_string(root.join("sshd.log")).unwrap_or_default()
        );
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            break;
        }
        assert!(Instant::now() < deadline, "sshd did not start");
        std::thread::sleep(Duration::from_millis(25));
    }
    let known = root.join("known_hosts");
    let trust = |public_key: PathBuf| {
        let key = std::fs::read_to_string(public_key).unwrap();
        std::fs::write(&known, format!("[127.0.0.1]:{port} {key}")).unwrap();
    };
    // Trust the exact generated host public key without disabling verification.
    trust(host_key.with_extension("pub"));
    let username = std::env::var("USER").unwrap();
    let destination = SshDestination::new("127.0.0.1", &username, port).unwrap();
    let database = root.join("connections.db");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let plan = runtime.block_on(async {
        let mut store = HostStore::open(text(&database)).await.unwrap();
        let mut identity = NewIdentity::new(
            "Acceptance",
            &username,
            AuthMethod::PublicKey {
                key_name: "Acceptance".into(),
            },
        )
        .unwrap();
        identity.key_path = Some(text(&key).into());
        identity.passphrase_secret_ref = Some("env://SHELLUMI_ACCEPTANCE_KEY_PASSPHRASE".into());
        let identity_id = store.create_identity(&identity).await.unwrap();
        store
            .add_group(
                &HostGroup::new("Acceptance", None, None, &[])
                    .unwrap()
                    .with_identity_id(Some(identity_id)),
            )
            .await
            .unwrap();
        let host = store
            .add_full(
                "Local test server",
                &destination,
                Some("Acceptance"),
                &[],
                None,
                None,
                "ssh",
                None,
                None,
            )
            .await
            .unwrap();
        drop(store);
        shellumi_storage::desktop::resolve_connection(text(&database), host)
            .await
            .unwrap()
    });
    let mut options = SftpConnectOptions {
        ssh_config_file: Some(PathBuf::from("/dev/null")),
        port: Some(port),
        identity_file: plan.identity_file.clone(),
        certificate_file: plan.certificate_file.clone(),
        timeout_seconds: 5,
        extra_ssh_options: plan.authentication_options.clone(),
        environment: vec![
            (
                "SSH_ASKPASS".into(),
                env!("CARGO_BIN_EXE_shellumi-app").into(),
            ),
            ("SSH_ASKPASS_REQUIRE".into(), "force".into()),
            ("DISPLAY".into(), "shellumi:0".into()),
            ("SHELLUMI_ASKPASS_MODE".into(), "1".into()),
            (
                "SHELLUMI_ASKPASS_TARGET".into(),
                format!("{username}@127.0.0.1"),
            ),
            (
                "SHELLUMI_ASKPASS_KEY".into(),
                plan.identity_file.clone().unwrap(),
            ),
            (
                "SHELLUMI_ASKPASS_PASSPHRASE_REF".into(),
                plan.passphrase_ref.clone().unwrap(),
            ),
            ("SHELLUMI_ASKPASS_PASSWORD_REF".into(), String::new()),
            ("SSH_ASKPASS_PROMPT".into(), String::new()),
            (
                "SHELLUMI_ACCEPTANCE_KEY_PASSPHRASE".into(),
                PASSPHRASE.into(),
            ),
        ],
    };
    options.extra_ssh_options.extend([
        ("UserKnownHostsFile".into(), text(&known).into()),
        ("GlobalKnownHostsFile".into(), "/dev/null".into()),
        ("IdentityAgent".into(), "none".into()),
    ]);
    let mut client = SftpClient::connect(&plan.destination, &options).unwrap();
    assert_eq!(client.server_version(), 3);
    let remote_dir = root.join("remote files");
    client.mkdir(text(&remote_dir)).unwrap();
    let source = root.join("upload.bin");
    let remote = remote_dir.join("uploaded file.bin");
    let downloaded = root.join("downloaded.bin");
    let payload: Vec<u8> = (0..131_079).map(|index| (index % 251) as u8).collect();
    std::fs::write(&source, &payload).unwrap();
    assert_eq!(
        client
            .upload_file(&source, text(&remote), None::<fn(u64, u64)>)
            .unwrap(),
        payload.len() as u64
    );
    let entries = client.list_remote_directory(text(&remote_dir)).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "uploaded file.bin");
    assert_eq!(
        client
            .download_file(text(&remote), &downloaded, None::<fn(u64, u64)>)
            .unwrap(),
        payload.len() as u64
    );
    assert_eq!(std::fs::read(&downloaded).unwrap(), payload);
    assert!(client
        .list_remote_directory(text(&root.join("missing")))
        .is_err());
    // Empty files must still be created, transferred and closed correctly.
    std::fs::write(&source, []).unwrap();
    assert_eq!(
        client
            .upload_file(&source, text(&remote), None::<fn(u64, u64)>)
            .unwrap(),
        0
    );
    assert_eq!(
        client
            .download_file(text(&remote), &downloaded, None::<fn(u64, u64)>)
            .unwrap(),
        0
    );
    assert!(std::fs::read(&downloaded).unwrap().is_empty());
    drop(client);
    let mut client = SftpClient::connect(&plan.destination, &options).unwrap();
    client.remove_file(text(&remote)).unwrap();
    client.rmdir(text(&remote_dir)).unwrap();
    drop(client);
    trust(wrong_key.with_extension("pub"));
    assert!(
        SftpClient::connect(&plan.destination, &options).is_err(),
        "Changed server key must fail closed"
    );
    std::fs::write(&known, "").unwrap();
    assert!(
        SftpClient::connect(&plan.destination, &options).is_err(),
        "Unknown server key must fail closed"
    );
    trust(host_key.with_extension("pub"));
    options
        .environment
        .iter_mut()
        .find(|(key, _)| key == "SHELLUMI_ACCEPTANCE_KEY_PASSPHRASE")
        .unwrap()
        .1 = "incorrect-fixture-passphrase".into();
    assert!(
        SftpClient::connect(&plan.destination, &options).is_err(),
        "Incorrect key passphrase must fail"
    );
    println!("Real OpenSSH: group identity, encrypted key, SFTP listing, binary/empty transfers, reconnect, changed/unknown host key rejection and wrong-passphrase rejection passed.");
}

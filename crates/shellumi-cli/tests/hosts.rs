use std::process::Command;

#[test]
fn manages_hosts_across_processes() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("hosts.db");
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_shellumi-cli"))
            .arg(&db)
            .args(args)
            .output()
            .unwrap()
    };
    let added = run(&["add", "Server", "example.org", "alice", "22"]);
    assert!(added.status.success());
    let id = String::from_utf8(added.stdout).unwrap().trim().to_owned();
    assert!(run(&["update", &id, "New", "example.net", "bob", "2222"])
        .status
        .success());
    let listed = run(&["list"]);
    assert!(listed.status.success());
    assert_eq!(
        String::from_utf8(listed.stdout).unwrap(),
        format!("{id}\tNew\texample.net\tbob\t2222\t-\t-\t-\t-\n")
    );
    assert!(run(&["delete", &id]).status.success());
    assert!(!run(&["delete", &id]).status.success());
    assert!(run(&["list"]).stdout.is_empty());
}

#[test]
fn supports_groups_tags_and_search() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("hosts.db");
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_shellumi-cli"))
            .arg(&db)
            .args(args)
            .output()
            .unwrap()
    };

    let add1 = run(&[
        "add",
        "ProdWeb",
        "192.168.1.10",
        "admin",
        "22",
        "--group",
        "Production",
        "--tags",
        "web,primary",
    ]);
    assert!(add1.status.success());

    let add2 = run(&[
        "add",
        "StagingDB",
        "10.0.0.2",
        "postgres",
        "5432",
        "--group",
        "Staging",
        "--tags",
        "db,internal",
    ]);
    assert!(add2.status.success());

    // Filter by group
    let prod_list = run(&["list", "--group", "Production"]);
    let prod_out = String::from_utf8(prod_list.stdout).unwrap();
    assert!(prod_out.contains("ProdWeb"));
    assert!(!prod_out.contains("StagingDB"));

    // Filter by tag
    let db_list = run(&["list", "--tag", "db"]);
    let db_out = String::from_utf8(db_list.stdout).unwrap();
    assert!(db_out.contains("StagingDB"));
    assert!(!db_out.contains("ProdWeb"));

    // Search query
    let search_res = run(&["list", "--search", "192.168.1.10"]);
    let search_out = String::from_utf8(search_res.stdout).unwrap();
    assert!(search_out.contains("ProdWeb"));
    assert!(!search_out.contains("StagingDB"));
}

#[test]
fn keygen_command_outputs_public_key_and_fingerprint() {
    let res = Command::new(env!("CARGO_BIN_EXE_shellumi-cli"))
        .args(["keygen", "--comment", "test-key"])
        .output()
        .unwrap();
    assert!(res.status.success());
    let stdout = String::from_utf8(res.stdout).unwrap();
    assert!(stdout.contains("Generated Ed25519 SSH Key Pair:"));
    assert!(stdout.contains("Public Key:  ssh-ed25519 "));
    assert!(stdout.contains("test-key"));
    assert!(stdout.contains("Fingerprint: SHA256:"));
}

#[test]
fn invalid_command_does_not_create_database() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("hosts.db");
    let result = Command::new(env!("CARGO_BIN_EXE_shellumi-cli"))
        .arg(&db)
        .args(["add", "Server", "-bad", "alice", "22"])
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(!db.exists());
}

#[cfg(unix)]
#[test]
fn connect_requests_host_key_confirmation_without_shell_expansion() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("hosts.db");
    let fake_ssh = dir.path().join("ssh");
    std::fs::write(&fake_ssh, "#!/bin/sh\nprintf '<%s>\\n' \"$@\"\n").unwrap();
    std::fs::set_permissions(&fake_ssh, std::fs::Permissions::from_mode(0o700)).unwrap();

    let added = Command::new(env!("CARGO_BIN_EXE_shellumi-cli"))
        .arg(&db)
        .args(["add", "Test", "host;touch-pwned", "alice", "2222"])
        .output()
        .unwrap();
    assert!(added.status.success());
    let id = String::from_utf8(added.stdout).unwrap().trim().to_owned();
    let result = Command::new(env!("CARGO_BIN_EXE_shellumi-cli"))
        .arg(&db)
        .args(["connect", &id])
        .env("PATH", dir.path())
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(result.status.success());
    assert_eq!(
        String::from_utf8(result.stdout).unwrap(),
        "<-o>\n<StrictHostKeyChecking=ask>\n<-o>\n<ForwardAgent=no>\n<-p>\n<2222>\n<-l>\n<alice>\n<-->\n<host;touch-pwned>\n"
    );
    assert!(!dir.path().join("touch-pwned").exists());

    let missing = Command::new(env!("CARGO_BIN_EXE_shellumi-cli"))
        .arg(&db)
        .args(["connect", "99999"])
        .env("PATH", dir.path())
        .output()
        .unwrap();
    assert!(!missing.status.success());
    assert!(missing.stdout.is_empty());
}

#[cfg(unix)]
#[test]
fn quick_connect_dispatches_ssh_correctly() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("hosts.db");
    let fake_ssh = dir.path().join("ssh");
    std::fs::write(&fake_ssh, "#!/bin/sh\nprintf '<%s>\\n' \"$@\"\n").unwrap();
    std::fs::set_permissions(&fake_ssh, std::fs::Permissions::from_mode(0o700)).unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_shellumi-cli"))
        .arg(&db)
        .args(["quick-connect", "admin@192.168.1.100:2200"])
        .env("PATH", dir.path())
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(result.status.success());
    assert_eq!(
        String::from_utf8(result.stdout).unwrap(),
        "<-o>\n<StrictHostKeyChecking=ask>\n<-o>\n<ForwardAgent=no>\n<-p>\n<2200>\n<-l>\n<admin>\n<-->\n<192.168.1.100>\n"
    );
}

#[cfg(unix)]
#[test]
fn connect_reports_ssh_failure() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("hosts.db");
    let fake_ssh = dir.path().join("ssh");
    std::fs::write(&fake_ssh, "#!/bin/sh\nexit 7\n").unwrap();
    std::fs::set_permissions(&fake_ssh, std::fs::Permissions::from_mode(0o700)).unwrap();
    let added = Command::new(env!("CARGO_BIN_EXE_shellumi-cli"))
        .arg(&db)
        .args(["add", "Test", "example.org", "alice", "22"])
        .output()
        .unwrap();
    assert!(added.status.success());
    let id = String::from_utf8(added.stdout).unwrap().trim().to_owned();
    let result = Command::new(env!("CARGO_BIN_EXE_shellumi-cli"))
        .arg(&db)
        .args(["connect", &id])
        .env("PATH", dir.path())
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8(result.stderr)
        .unwrap()
        .contains("OpenSSH exited"));
}

#[cfg(unix)]
#[test]
fn connect_places_jump_before_destination() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("hosts.db");
    let fake_ssh = dir.path().join("ssh");
    std::fs::write(&fake_ssh, "#!/bin/sh\nprintf '<%s>\\n' \"$@\"\n").unwrap();
    std::fs::set_permissions(&fake_ssh, std::fs::Permissions::from_mode(0o700)).unwrap();
    let added = Command::new(env!("CARGO_BIN_EXE_shellumi-cli"))
        .arg(&db)
        .args([
            "add",
            "Target",
            "example.org",
            "alice",
            "22",
            "--jump-host",
            "bob@jump.example:2222",
        ])
        .output()
        .unwrap();
    assert!(added.status.success());
    let id = String::from_utf8(added.stdout).unwrap().trim().to_owned();
    let connected = Command::new(env!("CARGO_BIN_EXE_shellumi-cli"))
        .arg(&db)
        .args(["connect", &id])
        .env("PATH", dir.path())
        .output()
        .unwrap();
    assert!(connected.status.success());
    let arguments = String::from_utf8(connected.stdout).unwrap();
    assert!(
        arguments.ends_with("<-J>\n<bob@jump.example:2222>\n<-->\n<example.org>\n"),
        "{arguments}"
    );
}

#[test]
fn vault_export_requires_an_explicit_passphrase() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("hosts.db");
    let vault = dir.path().join("vault.json");
    let result = Command::new(env!("CARGO_BIN_EXE_shellumi-cli"))
        .arg(&db)
        .args(["vault", "export"])
        .arg(&vault)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(!vault.exists());
    assert!(String::from_utf8(result.stderr)
        .unwrap()
        .contains("passphrase"));
}

#[test]
fn batch_execution_fails_instead_of_claiming_mock_success() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("hosts.db");
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_shellumi-cli"))
            .arg(&db)
            .args(args)
            .output()
            .unwrap()
    };
    assert!(run(&["add", "Test", "example.org", "alice", "22"])
        .status
        .success());
    let snippet = run(&["snippet", "add", "Whoami", "whoami"]);
    assert!(snippet.status.success());
    let output = String::from_utf8(snippet.stdout).unwrap();
    let id = output
        .split_whitespace()
        .find_map(|part| {
            part.trim_matches(|c: char| !c.is_ascii_digit())
                .parse::<i64>()
                .ok()
        })
        .expect("snippet ID");
    let id = id.to_string();
    let result = run(&["snippet", "batch", &id, "--hosts", "Test"]);
    assert!(!result.status.success());
    assert!(String::from_utf8(result.stderr)
        .unwrap()
        .contains("not implemented"));
    assert!(!String::from_utf8(result.stdout)
        .unwrap()
        .contains("Execution complete"));
    assert!(
        run(&["snippet", "batch", &id, "--hosts", "Test", "--dry-run"])
            .status
            .success()
    );
}

#[test]
fn imports_and_exports_ssh_config() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("hosts.db");
    let ssh_config = dir.path().join("config");

    let sample_config = r#"
Host web-prod
    HostName 198.51.100.1
    User admin
    Port 2200

Host db-staging
    HostName 10.0.0.5
    User postgres
    Port 5432
"#;
    std::fs::write(&ssh_config, sample_config).unwrap();

    let import_res = Command::new(env!("CARGO_BIN_EXE_shellumi-cli"))
        .arg(&db)
        .args(["import-ssh-config", ssh_config.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(import_res.status.success());
    assert!(String::from_utf8(import_res.stdout)
        .unwrap()
        .contains("Imported 2 host(s) from SSH config."));

    let export_res = Command::new(env!("CARGO_BIN_EXE_shellumi-cli"))
        .arg(&db)
        .arg("export-ssh-config")
        .output()
        .unwrap();
    assert!(export_res.status.success());
    let exported = String::from_utf8(export_res.stdout).unwrap();
    assert!(exported.contains("Host web-prod"));
    assert!(exported.contains("HostName 198.51.100.1"));
    assert!(exported.contains("Port 2200"));
    assert!(exported.contains("Host db-staging"));
    assert!(exported.contains("HostName 10.0.0.5"));
    assert!(exported.contains("Port 5432"));
}

#[test]
fn imports_and_exports_csv() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("hosts.db");
    let csv_file = dir.path().join("hosts.csv");

    let sample_csv = "name,hostname,username,port\n\"Cluster Master\",192.168.1.1,root,22\n";
    std::fs::write(&csv_file, sample_csv).unwrap();

    let import_res = Command::new(env!("CARGO_BIN_EXE_shellumi-cli"))
        .arg(&db)
        .args(["import-csv", csv_file.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(import_res.status.success());
    assert!(String::from_utf8(import_res.stdout)
        .unwrap()
        .contains("Imported 1 host(s) from CSV."));

    let export_res = Command::new(env!("CARGO_BIN_EXE_shellumi-cli"))
        .arg(&db)
        .arg("export-csv")
        .output()
        .unwrap();
    assert!(export_res.status.success());
    let exported = String::from_utf8(export_res.stdout).unwrap();
    assert!(exported.contains("name,hostname,username,port"));
    assert!(exported.contains("Cluster Master,192.168.1.1,root,22"));
}

#[test]
fn manages_groups_via_cli() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("hosts.db");
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_shellumi-cli"))
            .arg(&db)
            .args(args)
            .output()
            .unwrap()
    };

    // Add a group
    let add_res = run(&[
        "group",
        "add",
        "Infrastructure",
        "--username",
        "devops",
        "--port",
        "2222",
        "--tags",
        "cloud,aws",
    ]);
    assert!(add_res.status.success());
    assert!(String::from_utf8(add_res.stdout)
        .unwrap()
        .contains("Group 'Infrastructure' added."));

    // List groups
    let list_res = run(&["group", "list"]);
    assert!(list_res.status.success());
    let list_out = String::from_utf8(list_res.stdout).unwrap();
    assert!(list_out.contains("Infrastructure\tdevops\t2222\tcloud,aws"));

    // Show group details
    let show_res = run(&["group", "show", "Infrastructure"]);
    assert!(show_res.status.success());
    let show_out = String::from_utf8(show_res.stdout).unwrap();
    assert!(show_out.contains("Name:             Infrastructure"));
    assert!(show_out.contains("Default Username: devops"));
    assert!(show_out.contains("Default Port:     2222"));
    assert!(show_out.contains("Tags:             cloud, aws"));

    // Delete group
    let del_res = run(&["group", "delete", "Infrastructure"]);
    assert!(del_res.status.success());
    assert!(String::from_utf8(del_res.stdout)
        .unwrap()
        .contains("Group 'Infrastructure' deleted."));

    // List is now empty
    let list_after = run(&["group", "list"]);
    assert!(list_after.status.success());
    assert!(list_after.stdout.is_empty());
}

#[test]
fn manages_jump_hosts_and_tunnels_via_cli() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("hosts.db");
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_shellumi-cli"))
            .arg(&db)
            .args(args)
            .output()
            .unwrap()
    };

    // Add host with jump host
    let add_host = run(&[
        "add",
        "BastionHost",
        "10.0.0.50",
        "root",
        "22",
        "--jump-host",
        "jump.corp.internal:2222",
    ]);
    assert!(add_host.status.success());
    let host_id = String::from_utf8(add_host.stdout)
        .unwrap()
        .trim()
        .to_string();

    // Verify list includes jump host
    let list_host = run(&["list"]);
    assert!(list_host.status.success());
    let list_out = String::from_utf8(list_host.stdout).unwrap();
    assert!(list_out.contains("jump.corp.internal:2222"));

    // Add a local tunnel
    let add_tun1 = run(&[
        "tunnel",
        "add",
        "Postgres Forward",
        "--type",
        "local",
        "--bind-port",
        "5432",
        "--target-host",
        "internal-db",
        "--target-port",
        "5432",
        "--host-id",
        &host_id,
        "--auto-start",
    ]);
    assert!(add_tun1.status.success());
    let tun1_id = String::from_utf8(add_tun1.stdout)
        .unwrap()
        .trim()
        .to_string();

    // Add a dynamic tunnel (SOCKS)
    let add_tun2 = run(&[
        "tunnel",
        "add",
        "Corp SOCKS",
        "--type",
        "dynamic",
        "--bind-port",
        "1080",
    ]);
    assert!(add_tun2.status.success());
    let tun2_id = String::from_utf8(add_tun2.stdout)
        .unwrap()
        .trim()
        .to_string();

    // List tunnels
    let list_tunnels = run(&["tunnel", "list"]);
    assert!(list_tunnels.status.success());
    let tun_out = String::from_utf8(list_tunnels.stdout).unwrap();
    assert!(tun_out.contains("Postgres Forward"));
    assert!(tun_out.contains("-L"));
    assert!(tun_out.contains("5432:internal-db:5432"));
    assert!(tun_out.contains("auto"));
    assert!(tun_out.contains("Corp SOCKS"));
    assert!(tun_out.contains("-D"));

    // Delete tunnel
    let del_tun = run(&["tunnel", "delete", &tun1_id]);
    assert!(del_tun.status.success());
    assert!(String::from_utf8(del_tun.stdout)
        .unwrap()
        .contains("deleted"));

    let list_after = run(&["tunnel", "list"]);
    let out_after = String::from_utf8(list_after.stdout).unwrap();
    assert!(!out_after.contains("Postgres Forward"));
    assert!(out_after.contains("Corp SOCKS"));

    // Delete tunnel 2
    let del_tun2 = run(&["tunnel", "delete", &tun2_id]);
    assert!(del_tun2.status.success());
    assert!(run(&["tunnel", "list"]).stdout.is_empty());
}

#[test]
fn supports_sftp_local_ls_and_transfer_history() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("hosts.db");
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_shellumi-cli"))
            .arg(&db)
            .args(args)
            .output()
            .unwrap()
    };

    // Test local-ls
    let files_dir = dir.path().join("files");
    std::fs::create_dir(&files_dir).unwrap();
    std::fs::create_dir(files_dir.join("subfolder")).unwrap();
    std::fs::write(files_dir.join("test.txt"), "hello world").unwrap();
    std::fs::write(files_dir.join(".secret"), "hidden").unwrap();

    let ls_out = run(&["sftp", "local-ls", files_dir.to_str().unwrap()]);
    assert!(ls_out.status.success());
    let ls_str = String::from_utf8(ls_out.stdout).unwrap();
    assert!(ls_str.contains("subfolder"));
    assert!(ls_str.contains("test.txt"));
    assert!(!ls_str.contains(".secret"));

    let ls_all = run(&["sftp", "local-ls", files_dir.to_str().unwrap(), "--all"]);
    assert!(ls_all.status.success());
    let ls_all_str = String::from_utf8(ls_all.stdout).unwrap();
    assert!(ls_all_str.contains(".secret"));

    // Add a host for transfer association
    let added = run(&["add", "SftpHost", "sftp.example.org", "deploy", "22"]);
    assert!(added.status.success());
    let host_id = String::from_utf8(added.stdout).unwrap().trim().to_owned();

    // Log transfers
    let log1 = run(&[
        "sftp",
        "log",
        "upload",
        "/local/app.tar.gz",
        "/remote/app.tar.gz",
        "1048576",
        "--total-bytes",
        "1048576",
        "--host-id",
        &host_id,
        "--status",
        "completed",
    ]);
    assert!(log1.status.success());
    let log1_id = String::from_utf8(log1.stdout).unwrap().trim().to_owned();
    assert!(!log1_id.is_empty());

    let log2 = run(&[
        "sftp",
        "log",
        "download",
        "/local/backup.sql",
        "/remote/backup.sql",
        "512",
        "--total-bytes",
        "1024",
        "--status",
        "in_progress",
    ]);
    assert!(log2.status.success());

    // History listing
    let hist = run(&["sftp", "history"]);
    assert!(hist.status.success());
    let hist_str = String::from_utf8(hist.stdout).unwrap();
    assert!(hist_str.contains("Upload"));
    assert!(hist_str.contains("Download"));
    assert!(hist_str.contains("/local/app.tar.gz"));
    assert!(hist_str.contains("/remote/backup.sql"));

    // Filter by host
    let host_hist = run(&["sftp", "history", "--host-id", &host_id]);
    assert!(host_hist.status.success());
    let host_hist_str = String::from_utf8(host_hist.stdout).unwrap();
    assert!(host_hist_str.contains("/local/app.tar.gz"));
    assert!(!host_hist_str.contains("/local/backup.sql"));

    // Limit
    let limit_hist = run(&["sftp", "history", "--limit", "1"]);
    assert!(limit_hist.status.success());
    let limit_lines: Vec<_> = String::from_utf8(limit_hist.stdout)
        .unwrap()
        .lines()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(limit_lines.len(), 1);

    // Clear history
    let clear_host = run(&["sftp", "clear-history", "--host-id", &host_id]);
    assert!(clear_host.status.success());
    let after_clear = run(&["sftp", "history", "--host-id", &host_id]);
    assert!(String::from_utf8(after_clear.stdout).unwrap().is_empty());

    let clear_all = run(&["sftp", "clear-history"]);
    assert!(clear_all.status.success());
    assert!(run(&["sftp", "history"]).stdout.is_empty());
}

#[test]
fn supports_sftp_packet_codecs_and_transfer_queue() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("hosts.db");
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_shellumi-cli"))
            .arg(&db)
            .args(args)
            .output()
            .unwrap()
    };

    // 1. Encode Init
    let enc_init = run(&["sftp", "packet", "encode-init", "--version", "3"]);
    assert!(enc_init.status.success());
    let init_out = String::from_utf8(enc_init.stdout).unwrap();
    assert!(init_out.contains("Packet: Init (version: 3)"));
    assert!(init_out.contains("000000050100000003"));

    // 2. Decode Init hex
    let dec_init = run(&["sftp", "packet", "decode", "000000050100000003"]);
    assert!(dec_init.status.success());
    let dec_out = String::from_utf8(dec_init.stdout).unwrap();
    assert!(dec_out.contains("Type: SSH_FXP_INIT (1)"));
    assert!(dec_out.contains("Length: 9 bytes"));
    assert!(dec_out.contains("Init { version: 3 }"));

    // 3. Encode Open packet
    let enc_open = run(&[
        "sftp",
        "packet",
        "encode-open",
        "77",
        "/var/log/audit.log",
        "--write",
        "--create",
        "--truncate",
    ]);
    assert!(enc_open.status.success());
    let open_out = String::from_utf8(enc_open.stdout).unwrap();
    assert!(open_out.contains("Packet: Open (id: 77"));
    assert!(open_out.contains("Hex: "));

    let hex_line = open_out
        .lines()
        .find(|l| l.starts_with("Hex: "))
        .unwrap()
        .trim_start_matches("Hex: ")
        .trim();

    // 4. Decode Open packet from its generated hex
    let dec_open = run(&["sftp", "packet", "decode", hex_line]);
    assert!(dec_open.status.success());
    let dec_open_out = String::from_utf8(dec_open.stdout).unwrap();
    assert!(dec_open_out.contains("Type: SSH_FXP_OPEN (3)"));
    assert!(dec_open_out.contains("Request ID: 77"));
    assert!(dec_open_out.contains("/var/log/audit.log"));

    // 5. Transfer queue simulation
    let q_res = run(&[
        "sftp",
        "queue",
        "upload",
        "/local/data.tar",
        "/remote/data.tar",
        "2048",
    ]);
    assert!(q_res.status.success());
    let q_out = String::from_utf8(q_res.stdout).unwrap();
    assert!(q_out.contains("Task ID: 1"));
    assert!(q_out.contains("Transferred: 2048/2048 bytes (100.0%)"));
    assert!(q_out.contains("Completed: 1/1"));
}

#[test]
fn supports_snippet_management_and_variable_rendering() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("hosts.db");
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_shellumi-cli"))
            .arg(&db)
            .args(args)
            .output()
            .unwrap()
    };

    // Add snippet 1
    let add1 = run(&[
        "snippet",
        "add",
        "Deploy Service",
        "systemctl restart {{service:-nginx}} && journalctl -u {{service:-nginx}} -n 50",
        "--description",
        "Restarts and checks systemd service",
        "--tags",
        "systemd,ops",
    ]);
    assert!(add1.status.success());
    let id1 = String::from_utf8(add1.stdout).unwrap().trim().to_string();

    // Add snippet 2 with secret
    let add2 = run(&[
        "snippet",
        "add",
        "Postgres Backup",
        "PGPASSWORD={{db_password}} pg_dump -h {{db_host:-localhost}} -U {{user}} {{db_name}}",
        "--description",
        "Dumps database securely",
        "--tags",
        "db,backup",
    ]);
    assert!(add2.status.success());
    let id2 = String::from_utf8(add2.stdout).unwrap().trim().to_string();

    // List snippets
    let list_out = run(&["snippet", "list"]);
    assert!(list_out.status.success());
    let list_str = String::from_utf8(list_out.stdout).unwrap();
    assert!(list_str.contains("Deploy Service"));
    assert!(list_str.contains("Postgres Backup"));
    assert!(list_str.contains("systemd,ops"));

    // Filter by tag
    let tag_out = run(&["snippet", "list", "--tag", "db"]);
    assert!(tag_out.status.success());
    let tag_str = String::from_utf8(tag_out.stdout).unwrap();
    assert!(tag_str.contains("Postgres Backup"));
    assert!(!tag_str.contains("Deploy Service"));

    // Search
    let search_out = run(&["snippet", "list", "--search", "service"]);
    assert!(search_out.status.success());
    let search_str = String::from_utf8(search_out.stdout).unwrap();
    assert!(search_str.contains("Deploy Service"));
    assert!(!search_str.contains("Postgres Backup"));

    // Show snippet
    let show_out = run(&["snippet", "show", &id2]);
    assert!(show_out.status.success());
    let show_str = String::from_utf8(show_out.stdout).unwrap();
    assert!(show_str.contains("Postgres Backup"));
    assert!(show_str.contains("db_password"));
    assert!(show_str.contains("[SECRET]"));
    assert!(show_str.contains("db_host"));

    // Render snippet with default
    let render_def = run(&["snippet", "render", &id1]);
    assert!(render_def.status.success());
    assert_eq!(
        String::from_utf8(render_def.stdout).unwrap().trim(),
        "systemctl restart nginx && journalctl -u nginx -n 50"
    );

    // Render snippet with custom var
    let render_custom = run(&["snippet", "render", &id1, "--var", "service=postgresql"]);
    assert!(render_custom.status.success());
    assert_eq!(
        String::from_utf8(render_custom.stdout).unwrap().trim(),
        "systemctl restart postgresql && journalctl -u postgresql -n 50"
    );

    // Render preview with secret redaction
    let preview_out = run(&[
        "snippet",
        "render",
        &id2,
        "--var",
        "user=alice",
        "--var",
        "db_name=prod_db",
        "--var",
        "db_password=super_secret_pass",
        "--preview",
    ]);
    assert!(preview_out.status.success());
    let prev_str = String::from_utf8(preview_out.stdout).unwrap();
    assert!(prev_str.contains("PGPASSWORD=[REDACTED] pg_dump -h localhost -U alice prod_db"));

    // Delete snippet
    let del1 = run(&["snippet", "delete", &id1]);
    assert!(del1.status.success());
    let list_after = run(&["snippet", "list"]);
    let list_after_str = String::from_utf8(list_after.stdout).unwrap();
    assert!(!list_after_str.contains("Deploy Service"));
    assert!(list_after_str.contains("Postgres Backup"));
}

#[test]
fn supports_vault_export_status_and_import() {
    let dir = tempfile::tempdir().unwrap();
    let db1 = dir.path().join("db1.db");
    let db2 = dir.path().join("db2.db");
    let vault_file = dir.path().join("backup.vault.json");

    let run_db1 = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_shellumi-cli"))
            .arg(&db1)
            .args(args)
            .output()
            .unwrap()
    };

    let run_db2 = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_shellumi-cli"))
            .arg(&db2)
            .args(args)
            .output()
            .unwrap()
    };

    // Populate db1
    let add_grp = run_db1(&[
        "group",
        "add",
        "EdgeFleet",
        "--username",
        "edge",
        "--port",
        "22",
    ]);
    assert!(add_grp.status.success());

    let add_host = run_db1(&[
        "add",
        "EdgeDevice1",
        "192.168.10.5",
        "edge",
        "22",
        "--group",
        "EdgeFleet",
        "--tags",
        "iot,arm64",
    ]);
    assert!(add_host.status.success());

    let add_snip = run_db1(&[
        "snippet",
        "add",
        "Reboot Device",
        "sudo reboot",
        "--description",
        "Hard reboot edge device",
    ]);
    assert!(add_snip.status.success());

    // Check status
    let status_out = run_db1(&["vault", "status"]);
    assert!(status_out.status.success());
    let status_str = String::from_utf8(status_out.stdout).unwrap();
    assert!(status_str.contains("Hosts:    1"));
    assert!(status_str.contains("Groups:   1"));
    assert!(status_str.contains("Snippets: 1"));

    // Export vault
    let exp_out = run_db1(&[
        "vault",
        "export",
        vault_file.to_str().unwrap(),
        "--passphrase",
        "secure-passphrase-77",
        "--iterations",
        "2000",
    ]);
    assert!(exp_out.status.success());
    let exp_str = String::from_utf8(exp_out.stdout).unwrap();
    assert!(exp_str.contains("Vault exported successfully"));

    assert!(vault_file.exists());
    let vault_json = std::fs::read_to_string(&vault_file).unwrap();
    assert!(vault_json.contains("vault_id"));
    assert!(vault_json.contains("records"));

    // Check diff on empty db2 before import: all records should be remote_only
    let pre_diff = run_db2(&[
        "vault",
        "diff",
        vault_file.to_str().unwrap(),
        "--passphrase",
        "secure-passphrase-77",
    ]);
    assert!(pre_diff.status.success());
    let pre_diff_str = String::from_utf8(pre_diff.stdout).unwrap();
    assert!(pre_diff_str.contains("remote_only"));
    assert!(pre_diff_str.contains("EdgeDevice1:192.168.10.5"));

    // Import into db2
    let imp_out = run_db2(&[
        "vault",
        "import",
        vault_file.to_str().unwrap(),
        "--passphrase",
        "secure-passphrase-77",
    ]);
    assert!(imp_out.status.success());
    let imp_str = String::from_utf8(imp_out.stdout).unwrap();
    assert!(imp_str.contains("Hosts imported:    1"));
    assert!(imp_str.contains("Groups imported:   1"));
    assert!(imp_str.contains("Snippets imported: 1"));

    // Check diff on db2 after import: records should now be in-sync / identical
    let post_diff = run_db2(&[
        "vault",
        "diff",
        vault_file.to_str().unwrap(),
        "--passphrase",
        "secure-passphrase-77",
    ]);
    assert!(post_diff.status.success());
    let post_diff_str = String::from_utf8(post_diff.stdout).unwrap();
    assert!(post_diff_str.contains("identical"));

    // Verify OAuth URL generation command
    let oauth_out = run_db1(&["vault", "oauth-url"]);
    assert!(oauth_out.status.success());
    let oauth_str = String::from_utf8(oauth_out.stdout).unwrap();
    assert!(oauth_str.contains("Code Verifier:"));
    assert!(oauth_str.contains("Code Challenge:"));
    assert!(oauth_str.contains("drive.appdata"));

    // Verify db2 has the imported host
    let list_hosts = run_db2(&["list"]);
    assert!(list_hosts.status.success());
    let list_str = String::from_utf8(list_hosts.stdout).unwrap();
    assert!(list_str.contains("EdgeDevice1"));
    assert!(list_str.contains("192.168.10.5"));

    // Verify db2 has the imported snippet
    let list_snips = run_db2(&["snippet", "list"]);
    assert!(list_snips.status.success());
    let snip_str = String::from_utf8(list_snips.stdout).unwrap();
    assert!(snip_str.contains("Reboot Device"));
}

#[test]
fn supports_vault_disaster_recovery_history_and_rollback() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("dr.db");
    let vault_file = dir.path().join("dr_vault.json");

    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_shellumi-cli"))
            .arg(&db)
            .args(args)
            .output()
            .unwrap()
    };

    // Add host
    let add_host = run(&[
        "add",
        "ProdGateway",
        "192.168.1.100",
        "root",
        "22",
        "--tags",
        "gateway,prod",
    ]);
    assert!(add_host.status.success());

    // Add snippet
    let add_snip = run(&["snippet", "add", "Uptime", "uptime"]);
    assert!(add_snip.status.success());

    // Export vault with custom passphrase (automatically archives snapshot)
    let exp = run(&[
        "vault",
        "export",
        vault_file.to_str().unwrap(),
        "--passphrase",
        "dr-super-secret-key",
        "--iterations",
        "2000",
    ]);
    assert!(exp.status.success());

    // Check vault history
    let hist = run(&["vault", "history"]);
    assert!(hist.status.success());
    let hist_str = String::from_utf8(hist.stdout).unwrap();
    assert!(hist_str.contains("SNAPSHOT ID"));
    assert!(hist_str.contains("RECORDS"));

    // Extract snapshot id from history output
    let lines: Vec<&str> = hist_str.lines().collect();
    assert!(lines.len() >= 2);
    let row_parts: Vec<&str> = lines[1].split('\t').collect();
    let snapshot_id = row_parts[1];

    // Delete host
    let del = run(&["delete", "1"]);
    assert!(del.status.success());

    // Verify host is gone
    let list_after_del = run(&["list"]);
    assert!(list_after_del.status.success());
    assert!(!String::from_utf8(list_after_del.stdout)
        .unwrap()
        .contains("ProdGateway"));

    // Rollback using archived snapshot ID
    let rb = run(&[
        "vault",
        "rollback",
        snapshot_id,
        "--passphrase",
        "dr-super-secret-key",
    ]);
    assert!(rb.status.success());
    let rb_str = String::from_utf8(rb.stdout).unwrap();
    assert!(rb_str.contains("Rolled back to snapshot"));
    assert!(rb_str.contains("Hosts imported:    1"));

    // Verify host is restored
    let list_after_rb = run(&["list"]);
    assert!(list_after_rb.status.success());
    let list_str = String::from_utf8(list_after_rb.stdout).unwrap();
    assert!(list_str.contains("ProdGateway"));
    assert!(list_str.contains("192.168.1.100"));
}

#[test]
fn supports_command_palette_search_and_batch_snippet_execution() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("hosts.db");
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_shellumi-cli"))
            .arg(&db)
            .args(args)
            .output()
            .unwrap()
    };

    // 1. Add hosts
    assert!(run(&[
        "add",
        "web-prod-01",
        "10.0.0.1",
        "ubuntu",
        "22",
        "--tags",
        "production,web"
    ])
    .status
    .success());
    assert!(run(&[
        "add",
        "web-prod-02",
        "10.0.0.2",
        "ubuntu",
        "22",
        "--tags",
        "production,web"
    ])
    .status
    .success());

    // 2. Add snippet
    let add_snip = run(&[
        "snippet",
        "add",
        "Cluster Status",
        "echo 'Node={{host.name}} IP={{host.hostname}} Env={{cluster_env:-production}}'",
        "--description",
        "Check cluster node state",
        "--tags",
        "cluster,ops",
    ]);
    assert!(add_snip.status.success());
    let snip_id = String::from_utf8(add_snip.stdout)
        .unwrap()
        .trim()
        .to_string();

    // 3. Command Palette search for hosts
    let pal_host = run(&["palette", "search", "web-prod"]);
    assert!(pal_host.status.success());
    let pal_host_out = String::from_utf8(pal_host.stdout).unwrap();
    assert!(pal_host_out.contains("Hosts"));
    assert!(pal_host_out.contains("Connect to web-prod-01"));
    assert!(pal_host_out.contains("Connect to web-prod-02"));

    // 4. Command Palette search for snippet
    let pal_snip = run(&["palette", "search", "Cluster"]);
    assert!(pal_snip.status.success());
    let pal_snip_out = String::from_utf8(pal_snip.stdout).unwrap();
    assert!(pal_snip_out.contains("Snippets"));
    assert!(pal_snip_out.contains("Run Snippet: Cluster Status"));

    // 5. Command Palette search for static action
    let pal_action = run(&["palette", "search", "Quick Connect"]);
    assert!(pal_action.status.success());
    let pal_action_out = String::from_utf8(pal_action.stdout).unwrap();
    assert!(pal_action_out.contains("Quick Connect"));
    assert!(pal_action_out.contains("Cmd+O"));

    // 6. Snippet Batch Dry-Run Preview
    let batch_dry = run(&[
        "snippet",
        "batch",
        &snip_id,
        "--hosts",
        "web-prod-01,web-prod-02",
        "--var",
        "cluster_env=staging",
        "--dry-run",
    ]);
    assert!(batch_dry.status.success());
    let batch_dry_out = String::from_utf8(batch_dry.stdout).unwrap();
    assert!(batch_dry_out.contains("Batch Snippet Preview: Cluster Status"));
    assert!(batch_dry_out.contains("Targets:  2 host(s)"));
    assert!(batch_dry_out.contains("Node=web-prod-01 IP=10.0.0.1 Env=staging"));
    assert!(batch_dry_out.contains("Node=web-prod-02 IP=10.0.0.2 Env=staging"));

    // 7. Non-preview CLI execution must not report simulated success.
    let batch_exec = run(&[
        "snippet",
        "batch",
        &snip_id,
        "--hosts",
        "web-prod-01,web-prod-02",
        "--var",
        "cluster_env=staging",
    ]);
    assert!(!batch_exec.status.success());
    assert!(String::from_utf8(batch_exec.stderr)
        .unwrap()
        .contains("not implemented"));
}

#[test]
fn supports_host_startup_snippets_and_show() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("hosts.db");
    let db_str = db.to_str().unwrap();

    let run = |args: &[&str]| {
        let mut full_args = vec![db_str];
        full_args.extend_from_slice(args);
        Command::new(env!("CARGO_BIN_EXE_shellumi-cli"))
            .args(&full_args)
            .output()
            .unwrap()
    };

    // 1. Add snippet
    let add_snip = run(&[
        "snippet",
        "add",
        "Motd",
        "uname -a && uptime",
        "--description",
        "Print system info on login",
    ]);
    assert!(add_snip.status.success());
    let snip_id = String::from_utf8(add_snip.stdout)
        .unwrap()
        .trim()
        .to_string();

    // 2. Add host with startup snippet
    let add_host = run(&[
        "add",
        "BastionProd",
        "bastion.example.com",
        "ops",
        "2222",
        "--tags",
        "prod,bastion",
        "--startup-snippet",
        &snip_id,
    ]);
    assert!(add_host.status.success());
    let host_id = String::from_utf8(add_host.stdout)
        .unwrap()
        .trim()
        .to_string();

    // 3. Show host
    let show_res = run(&["show", &host_id]);
    assert!(show_res.status.success());
    let show_out = String::from_utf8(show_res.stdout).unwrap();
    assert!(show_out.contains("Name:            BastionProd"));
    assert!(show_out.contains("Destination:     ops@bastion.example.com:2222"));
    assert!(show_out.contains("Tags:            prod,bastion"));
    assert!(show_out.contains(&format!("Startup Snippet: {snip_id} (Motd)")));

    // 4. List hosts includes snippet column
    let list_res = run(&["list"]);
    assert!(list_res.status.success());
    let list_out = String::from_utf8(list_res.stdout).unwrap();
    assert!(list_out.contains(&format!(
        "{host_id}\tBastionProd\tbastion.example.com\tops\t2222\t-\tprod,bastion\t-\t{snip_id}"
    )));

    // 5. Update host startup snippet
    let update_res = run(&[
        "update",
        &host_id,
        "BastionProdUpdated",
        "bastion.example.com",
        "ops",
        "2222",
        "--tags",
        "prod",
    ]);
    assert!(update_res.status.success());
    let show_updated = run(&["show", &host_id]);
    let show_up_out = String::from_utf8(show_updated.stdout).unwrap();
    assert!(show_up_out.contains("Name:            BastionProdUpdated"));
    assert!(show_up_out.contains("Startup Snippet: -"));
}

#[test]
fn supports_workspace_cli_lifecycle() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("workspaces.db");

    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_shellumi-cli"))
            .arg(&db)
            .args(args)
            .output()
            .unwrap()
    };

    // 1. Initial list is empty
    let empty_list = run(&["workspace", "list"]);
    assert!(empty_list.status.success());
    assert!(String::from_utf8(empty_list.stdout)
        .unwrap()
        .contains("No workspaces configured."));

    // 2. Add workspace
    let add_res = run(&[
        "workspace",
        "add",
        "SRE Ops Center",
        "--layout",
        "vertical",
        "--desc",
        "Core SRE monitoring layout",
        "--tags",
        "sre,ops",
    ]);
    assert!(add_res.status.success());
    let ws_id = String::from_utf8(add_res.stdout)
        .unwrap()
        .trim()
        .to_string();

    // 3. List contains newly added workspace
    let list_res = run(&["workspace", "list"]);
    assert!(list_res.status.success());
    let list_out = String::from_utf8(list_res.stdout).unwrap();
    assert!(list_out.contains(&ws_id));
    assert!(list_out.contains("SRE Ops Center"));
    assert!(list_out.contains("vertical"));
    assert!(list_out.contains("sre,ops"));

    // 4. Show workspace
    let show_res = run(&["workspace", "show", &ws_id]);
    assert!(show_res.status.success());
    let show_out = String::from_utf8(show_res.stdout).unwrap();
    assert!(show_out.contains("Name:          SRE Ops Center"));
    assert!(show_out.contains("Layout:        vertical"));
    assert!(show_out.contains("Description:   Core SRE monitoring layout"));
    assert!(show_out.contains("Tags:          sre, ops"));

    // 5. Duplicate workspace
    let dup_res = run(&["workspace", "duplicate", &ws_id, "SRE Ops Center Staging"]);
    assert!(dup_res.status.success());
    let dup_id = String::from_utf8(dup_res.stdout)
        .unwrap()
        .trim()
        .to_string();
    assert_ne!(dup_id, ws_id);

    // 6. Delete original workspace
    let del_res = run(&["workspace", "delete", &ws_id]);
    assert!(del_res.status.success());
    assert!(String::from_utf8(del_res.stdout)
        .unwrap()
        .contains(&format!("Deleted workspace {ws_id}.")));

    // 7. List now contains only duplicate
    let final_list = run(&["workspace", "list"]);
    let final_out = String::from_utf8(final_list.stdout).unwrap();
    assert!(!final_out.contains("SRE Ops Center\t"));
    assert!(final_out.contains("SRE Ops Center Staging"));
}

#[test]
fn supports_multi_protocol_cli_and_serial_ports() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("protocols.db");
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_shellumi-cli"))
            .arg(&db)
            .args(args)
            .output()
            .unwrap()
    };

    // 1. Discover serial ports CLI command
    let sp_res = Command::new(env!("CARGO_BIN_EXE_shellumi-cli"))
        .arg("serial-ports")
        .output()
        .unwrap();
    assert!(sp_res.status.success());

    // 2. Add Mosh host
    let add_mosh = run(&[
        "add",
        "MoshNode",
        "mosh.example.org",
        "dev",
        "22",
        "--protocol",
        "mosh",
        "--protocol-config",
        r#"{"predict_mode":"adaptive"}"#,
    ]);
    assert!(add_mosh.status.success());
    let mosh_id = String::from_utf8(add_mosh.stdout)
        .unwrap()
        .trim()
        .to_string();

    let show_mosh = run(&["show", &mosh_id]);
    assert!(show_mosh.status.success());
    let show_mosh_str = String::from_utf8(show_mosh.stdout).unwrap();
    assert!(show_mosh_str.contains("Protocol:        mosh"));
    assert!(show_mosh_str.contains(r#"Protocol Config: {"predict_mode":"adaptive"}"#));

    // 3. Add Telnet host and verify security warning in show
    let add_telnet = run(&[
        "add",
        "LegacySwitch",
        "192.168.1.1",
        "admin",
        "23",
        "--protocol",
        "telnet",
    ]);
    assert!(add_telnet.status.success());
    let telnet_id = String::from_utf8(add_telnet.stdout)
        .unwrap()
        .trim()
        .to_string();

    let show_telnet = run(&["show", &telnet_id]);
    assert!(show_telnet.status.success());
    let show_telnet_str = String::from_utf8(show_telnet.stdout).unwrap();
    assert!(show_telnet_str.contains("Protocol:        telnet"));
    assert!(show_telnet_str.contains("UNENCRYPTED PROTOCOL: Telnet transmits all credentials"));

    // 4. Add Serial host
    let add_serial = run(&[
        "add",
        "ConsolePort",
        "ttyUSB0",
        "console",
        "1",
        "--protocol",
        "serial",
        "--protocol-config",
        r#"{"baud_rate":115200,"data_bits":8,"parity":"none","stop_bits":1}"#,
    ]);
    assert!(add_serial.status.success());
    let serial_id = String::from_utf8(add_serial.stdout)
        .unwrap()
        .trim()
        .to_string();

    let show_serial = run(&["show", &serial_id]);
    assert!(show_serial.status.success());
    let show_serial_str = String::from_utf8(show_serial.stdout).unwrap();
    assert!(show_serial_str.contains("Protocol:        serial"));

    // 5. Update Telnet host to SSH
    let update_res = run(&[
        "update",
        &telnet_id,
        "ModernSwitch",
        "192.168.1.1",
        "admin",
        "22",
        "--protocol",
        "ssh",
    ]);
    assert!(update_res.status.success());

    let show_updated = run(&["show", &telnet_id]);
    assert!(show_updated.status.success());
    let show_updated_str = String::from_utf8(show_updated.stdout).unwrap();
    assert!(show_updated_str.contains("Protocol:        ssh"));
    assert!(!show_updated_str.contains("cleartext"));

    // 6. Invalid protocol fails
    let bad_proto = run(&[
        "add",
        "BadHost",
        "example.com",
        "root",
        "22",
        "--protocol",
        "ftp_unsupported",
    ]);
    assert!(!bad_proto.status.success());
    let bad_err = String::from_utf8(bad_proto.stderr).unwrap();
    assert!(bad_err.contains("Invalid connection protocol"));
}

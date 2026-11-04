use shellumi_core::{
    build_google_drive_auth_url, diff_sync_records_with_passphrase, export_csv, export_ssh_config,
    generate_ed25519_key, generate_pkce_challenge, list_local_directory, parse_csv,
    parse_quick_connect_target, parse_ssh_config, render_command, render_preview, ForwardingType,
    HostConfigEntry, HostGroup, PortForwardRule, Snippet, SshDestination, SyncVault,
    TransferDirection,
};
use shellumi_storage::{HostStore, NewTransfer};
use std::{collections::HashMap, env, error::Error, fs, io, process::Command};

const USAGE: &str = "Usage:
  shellumi-cli <database-path> list [--group <group>] [--tag <tag>] [--search <query>]
  shellumi-cli <database-path> add <name> <hostname> <username> <port> [--group <group>] [--tags <tag1,tag2>] [--jump-host <jump>]
  shellumi-cli <database-path> update <id> <name> <hostname> <username> <port> [--group <group>] [--tags <tag1,tag2>] [--jump-host <jump>]
  shellumi-cli <database-path> delete <id>
  shellumi-cli <database-path> group add <name> [--username <user>] [--port <port>] [--tags <tag1,tag2>]
  shellumi-cli <database-path> group list
  shellumi-cli <database-path> group show <name>
  shellumi-cli <database-path> group delete <name>
  shellumi-cli <database-path> tunnel add <name> --type <local|remote|dynamic> --bind-port <port> [--bind-address <addr>] [--target-host <host>] [--target-port <port>] [--host-id <id>] [--auto-start]
  shellumi-cli <database-path> tunnel list [--host-id <id>]
  shellumi-cli <database-path> tunnel delete <id>
  shellumi-cli <database-path> sftp local-ls <path> [--all]
  shellumi-cli <database-path> sftp history [--host-id <id>] [--limit <n>]
  shellumi-cli <database-path> sftp clear-history [--host-id <id>]
  shellumi-cli <database-path> sftp log <upload|download> <local-path> <remote-path> <bytes> [--total-bytes <bytes>] [--status <status>] [--host-id <id>]
  shellumi-cli <database-path> snippet list [--tag <tag>] [--search <query>]
  shellumi-cli <database-path> snippet add <name> <command> [--description <desc>] [--tags <tag1,tag2>]
  shellumi-cli <database-path> snippet show <id>
  shellumi-cli <database-path> snippet render <id> [--var key=value] [--preview]
  shellumi-cli <database-path> snippet delete <id>
  shellumi-cli <database-path> vault export <output-path> [--passphrase <pass>] [--device-id <id>] [--iterations <n>]
  shellumi-cli <database-path> vault import <input-path> [--passphrase <pass>]
  shellumi-cli <database-path> vault diff <input-path> [--passphrase <pass>]
  shellumi-cli <database-path> vault history
  shellumi-cli <database-path> vault rollback <snapshot-id> [--passphrase <pass>]
  shellumi-cli <database-path> vault status
  shellumi-cli <database-path> vault oauth-url [--client-id <id>] [--redirect-uri <uri>]
  shellumi-cli <database-path> connect <id>
  shellumi-cli <database-path> quick-connect <user@host[:port]>
  shellumi-cli <database-path> import-ssh-config <config-path>
  shellumi-cli <database-path> export-ssh-config
  shellumi-cli <database-path> import-csv <csv-path>
  shellumi-cli <database-path> export-csv
  shellumi-cli keygen [--comment <comment>]";

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn parse_destination(fields: &[String]) -> Result<SshDestination, Box<dyn Error>> {
    let port: u16 = fields[2]
        .parse()
        .map_err(|_| invalid("Port must be between 1 and 65535."))?;
    Ok(SshDestination::new(&fields[0], &fields[1], port)?)
}

fn extract_flag(args: &[String], flag: &str) -> Option<String> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == flag {
            return iter.next().cloned();
        }
    }
    None
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() || (args.len() == 1 && matches!(args[0].as_str(), "--help" | "-h")) {
        println!("{USAGE}");
        return Ok(());
    }

    if args[0] == "keygen" {
        let comment = extract_flag(&args[1..], "--comment");
        let (_keypair, key_meta) = generate_ed25519_key(comment.as_deref())?;
        println!("Generated Ed25519 SSH Key Pair:");
        println!("Public Key:  {}", key_meta.public_key);
        println!("Fingerprint: {}", key_meta.fingerprint);
        return Ok(());
    }

    if args.len() < 2 {
        return Err(invalid(USAGE).into());
    }

    let db_path = &args[0];
    let command = args[1].as_str();

    // Early command validation
    match command {
        "list" | "export-ssh-config" | "export-csv" => {}
        "import-ssh-config" | "import-csv" => {
            if args.len() < 3 {
                return Err(invalid(USAGE).into());
            }
        }
        "add" => {
            if args.len() < 6 {
                return Err(invalid(USAGE).into());
            }
            parse_destination(&[args[3].clone(), args[4].clone(), args[5].clone()])?;
        }
        "update" => {
            if args.len() < 7 {
                return Err(invalid(USAGE).into());
            }
            args[2]
                .parse::<i64>()
                .map_err(|_| invalid("ID must be an integer."))?;
            parse_destination(&[args[4].clone(), args[5].clone(), args[6].clone()])?;
        }
        "delete" | "connect" => {
            if args.len() < 3 {
                return Err(invalid(USAGE).into());
            }
            args[2]
                .parse::<i64>()
                .map_err(|_| invalid("ID must be an integer."))?;
        }
        "group" => {
            if args.len() < 3 {
                return Err(invalid(USAGE).into());
            }
            match args[2].as_str() {
                "list" => {}
                "add" | "show" | "delete" => {
                    if args.len() < 4 {
                        return Err(invalid(USAGE).into());
                    }
                }
                _ => return Err(invalid(USAGE).into()),
            }
        }
        "tunnel" => {
            if args.len() < 3 {
                return Err(invalid(USAGE).into());
            }
            match args[2].as_str() {
                "list" => {}
                "delete" => {
                    if args.len() < 4 {
                        return Err(invalid(USAGE).into());
                    }
                    args[3]
                        .parse::<i64>()
                        .map_err(|_| invalid("Tunnel ID must be an integer."))?;
                }
                "add" => {
                    if args.len() < 4 {
                        return Err(invalid(USAGE).into());
                    }
                }
                _ => return Err(invalid(USAGE).into()),
            }
        }
        "sftp" => {
            if args.len() < 3 {
                return Err(invalid(USAGE).into());
            }
            match args[2].as_str() {
                "local-ls" => {
                    if args.len() < 4 {
                        return Err(invalid(USAGE).into());
                    }
                }
                "history" | "clear-history" => {}
                "log" => {
                    if args.len() < 7 {
                        return Err(invalid(USAGE).into());
                    }
                }
                _ => return Err(invalid(USAGE).into()),
            }
        }
        "snippet" => {
            if args.len() < 3 {
                return Err(invalid(USAGE).into());
            }
            match args[2].as_str() {
                "list" => {}
                "add" => {
                    if args.len() < 5 {
                        return Err(invalid(USAGE).into());
                    }
                }
                "show" | "render" | "delete" => {
                    if args.len() < 4 {
                        return Err(invalid(USAGE).into());
                    }
                    args[3]
                        .parse::<i64>()
                        .map_err(|_| invalid("Snippet ID must be an integer."))?;
                }
                _ => return Err(invalid(USAGE).into()),
            }
        }
        "vault" => {
            if args.len() < 3 {
                return Err(invalid(USAGE).into());
            }
            match args[2].as_str() {
                "export" | "import" | "diff" | "rollback" => {
                    if args.len() < 4 {
                        return Err(invalid(USAGE).into());
                    }
                }
                "status" | "oauth-url" | "history" => {}
                _ => return Err(invalid(USAGE).into()),
            }
        }
        "quick-connect" => {
            if args.len() < 3 {
                return Err(invalid(USAGE).into());
            }
            parse_quick_connect_target(&args[2])?;
        }
        _ => return Err(invalid(USAGE).into()),
    }

    let mut store = HostStore::open(db_path).await?;
    match command {
        "list" => {
            let extra = &args[2..];
            let group_filter = extract_flag(extra, "--group");
            let tag_filter = extract_flag(extra, "--tag");
            let search_query = extract_flag(extra, "--search");

            let hosts = if let Some(query) = search_query {
                store.search(&query).await?
            } else if let Some(group) = group_filter {
                store.list_by_group(&group).await?
            } else if let Some(tag) = tag_filter {
                store.list_by_tag(&tag).await?
            } else {
                store.list().await?
            };

            for host in hosts {
                let group = host.group_name.as_deref().unwrap_or("-");
                let jump = host.jump_host.as_deref().unwrap_or("-");
                let tags = if host.tags.is_empty() {
                    "-".to_string()
                } else {
                    host.tags.join(",")
                };
                println!(
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    host.id,
                    host.name,
                    host.destination.host(),
                    host.destination.username(),
                    host.destination.port(),
                    group,
                    tags,
                    jump
                );
            }
        }
        "add" => {
            let destination = parse_destination(&args[3..6])?;
            let extra = &args[6..];
            let group = extract_flag(extra, "--group");
            let jump_host = extract_flag(extra, "--jump-host");
            let tags: Vec<String> = extract_flag(extra, "--tags")
                .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_default();

            println!(
                "{}",
                store
                    .add(
                        &args[2],
                        &destination,
                        group.as_deref(),
                        &tags,
                        jump_host.as_deref()
                    )
                    .await?
            );
        }
        "update" => {
            let id = args[2].parse()?;
            let destination = parse_destination(&args[4..7])?;
            let extra = &args[7..];
            let group = extract_flag(extra, "--group");
            let jump_host = extract_flag(extra, "--jump-host");
            let tags: Vec<String> = extract_flag(extra, "--tags")
                .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_default();

            if !store
                .update(
                    id,
                    &args[3],
                    &destination,
                    group.as_deref(),
                    &tags,
                    jump_host.as_deref(),
                )
                .await?
            {
                return Err(invalid("Host ID not found.").into());
            }
        }
        "delete" => {
            let id = args[2].parse()?;
            if !store.delete(id).await? {
                return Err(invalid("Host ID not found.").into());
            }
        }
        "connect" => {
            let id: i64 = args[2].parse()?;
            let host = store
                .list()
                .await?
                .into_iter()
                .find(|host| host.id == id)
                .ok_or_else(|| invalid("Host ID not found."))?;
            let effective_dest = store.get_effective_destination(&host).await?;
            let mut cmd = Command::new("ssh");
            cmd.args(effective_dest.openssh_arguments());
            if let Some(jump) = &host.jump_host {
                cmd.arg("-J").arg(jump);
            }
            let status = cmd.status()?;
            if !status.success() {
                return Err(io::Error::other(format!("OpenSSH exited with {status}")).into());
            }
        }
        "group" => {
            let sub = args[2].as_str();
            match sub {
                "list" => {
                    let groups = store.list_groups().await?;
                    for group in groups {
                        let user = group.default_username.as_deref().unwrap_or("-");
                        let port = group
                            .default_port
                            .map(|p| p.to_string())
                            .unwrap_or_else(|| "-".to_string());
                        let tags = if group.tags.is_empty() {
                            "-".to_string()
                        } else {
                            group.tags.join(",")
                        };
                        println!("{}\t{}\t{}\t{}", group.name, user, port, tags);
                    }
                }
                "add" => {
                    let name = &args[3];
                    let extra = &args[4..];
                    let username = extract_flag(extra, "--username");
                    let port: Option<u16> = extract_flag(extra, "--port")
                        .map(|p| {
                            p.parse()
                                .map_err(|_| invalid("Port must be between 1 and 65535."))
                        })
                        .transpose()?;
                    let tags: Vec<String> = extract_flag(extra, "--tags")
                        .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
                        .unwrap_or_default();
                    let group = HostGroup::new(name, username.as_deref(), port, &tags)?;
                    store.add_group(&group).await?;
                    println!("Group '{}' added.", group.name);
                }
                "show" => {
                    let name = &args[3];
                    if let Some(group) = store.get_group(name).await? {
                        println!("Name:             {}", group.name);
                        println!(
                            "Default Username: {}",
                            group.default_username.as_deref().unwrap_or("(none)")
                        );
                        println!(
                            "Default Port:     {}",
                            group
                                .default_port
                                .map(|p| p.to_string())
                                .unwrap_or_else(|| "(none)".to_string())
                        );
                        println!(
                            "Tags:             {}",
                            if group.tags.is_empty() {
                                "(none)".to_string()
                            } else {
                                group.tags.join(", ")
                            }
                        );
                    } else {
                        return Err(invalid("Group not found.").into());
                    }
                }
                "delete" => {
                    let name = &args[3];
                    if !store.delete_group(name).await? {
                        return Err(invalid("Group not found.").into());
                    }
                    println!("Group '{name}' deleted.");
                }
                _ => unreachable!(),
            }
        }
        "tunnel" => {
            let sub = args[2].as_str();
            match sub {
                "list" => {
                    let host_id: Option<i64> = extract_flag(&args[3..], "--host-id")
                        .map(|h| {
                            h.parse()
                                .map_err(|_| invalid("Host ID must be an integer."))
                        })
                        .transpose()?;
                    let tunnels = if let Some(hid) = host_id {
                        store.list_tunnels_for_host(hid).await?
                    } else {
                        store.list_tunnels().await?
                    };
                    for t in tunnels {
                        let (flag, spec) = t.rule.openssh_argument();
                        let auto = if t.auto_start { "auto" } else { "manual" };
                        let warning = if t.rule.is_exposed_beyond_localhost() {
                            " [EXPOSED]"
                        } else {
                            ""
                        };
                        println!(
                            "{}\t{}\t{}\t{}{}\t{}\t{}",
                            t.id, t.name, t.rule.forwarding_type, flag, warning, spec, auto
                        );
                    }
                }
                "add" => {
                    let name = &args[3];
                    let extra = &args[4..];
                    let ftype_str = extract_flag(extra, "--type").ok_or_else(|| {
                        invalid("Missing required flag --type <local|remote|dynamic>.")
                    })?;
                    let ftype = ForwardingType::from_str_loose(&ftype_str).ok_or_else(|| {
                        invalid("Invalid forwarding type. Choose local, remote, or dynamic.")
                    })?;
                    let bind_port: u16 = extract_flag(extra, "--bind-port")
                        .ok_or_else(|| invalid("Missing required flag --bind-port <port>."))?
                        .parse()
                        .map_err(|_| invalid("Bind port must be between 1 and 65535."))?;
                    let bind_address = extract_flag(extra, "--bind-address");
                    let host_id: Option<i64> = extract_flag(extra, "--host-id")
                        .map(|h| {
                            h.parse()
                                .map_err(|_| invalid("Host ID must be an integer."))
                        })
                        .transpose()?;
                    let auto_start = extract_flag(extra, "--auto-start").is_some()
                        || extra.iter().any(|a| a == "--auto-start");

                    let rule = match ftype {
                        ForwardingType::Dynamic => {
                            PortForwardRule::dynamic(bind_address.as_deref(), bind_port)?
                        }
                        ForwardingType::Local => {
                            let target_host =
                                extract_flag(extra, "--target-host").ok_or_else(|| {
                                    invalid("Missing required flag --target-host <host>.")
                                })?;
                            let target_port: u16 = extract_flag(extra, "--target-port")
                                .ok_or_else(|| {
                                    invalid("Missing required flag --target-port <port>.")
                                })?
                                .parse()
                                .map_err(|_| invalid("Target port must be between 1 and 65535."))?;
                            PortForwardRule::local(
                                bind_address.as_deref(),
                                bind_port,
                                &target_host,
                                target_port,
                            )?
                        }
                        ForwardingType::Remote => {
                            let target_host =
                                extract_flag(extra, "--target-host").ok_or_else(|| {
                                    invalid("Missing required flag --target-host <host>.")
                                })?;
                            let target_port: u16 = extract_flag(extra, "--target-port")
                                .ok_or_else(|| {
                                    invalid("Missing required flag --target-port <port>.")
                                })?
                                .parse()
                                .map_err(|_| invalid("Target port must be between 1 and 65535."))?;
                            PortForwardRule::remote(
                                bind_address.as_deref(),
                                bind_port,
                                &target_host,
                                target_port,
                            )?
                        }
                    };

                    if rule.is_exposed_beyond_localhost() {
                        eprintln!(
                            "SECURITY WARNING: Tunnel rule binds to a non-localhost address and exposes services beyond 127.0.0.1!"
                        );
                    }

                    let tid = store.add_tunnel(name, host_id, &rule, auto_start).await?;
                    println!("{tid}");
                }
                "delete" => {
                    let id: i64 = args[3].parse()?;
                    if !store.delete_tunnel(id).await? {
                        return Err(invalid("Tunnel ID not found.").into());
                    }
                    println!("Tunnel {id} deleted.");
                }
                _ => unreachable!(),
            }
        }
        "sftp" => {
            let sub = args[2].as_str();
            match sub {
                "local-ls" => {
                    let path = &args[3];
                    let show_hidden = args[4..].iter().any(|a| a == "--all" || a == "-a");
                    let entries = list_local_directory(path, show_hidden)?;
                    for entry in entries {
                        println!(
                            "{}\t{:>10}\t{}",
                            entry.formatted_permissions(),
                            entry.formatted_size(),
                            entry.name
                        );
                    }
                }
                "history" => {
                    let extra = &args[3..];
                    let host_id: Option<i64> = extract_flag(extra, "--host-id")
                        .map(|h| {
                            h.parse()
                                .map_err(|_| invalid("Host ID must be an integer."))
                        })
                        .transpose()?;
                    let limit: Option<usize> = extract_flag(extra, "--limit")
                        .map(|l| l.parse().map_err(|_| invalid("Limit must be an integer.")))
                        .transpose()?;
                    let transfers = store.list_transfers(host_id, limit).await?;
                    for t in transfers {
                        let host_str = t
                            .host_id
                            .map(|h| h.to_string())
                            .unwrap_or_else(|| "-".to_string());
                        println!(
                            "{}\t{}\t{}\t{}\t{}/{}\t{} -> {}",
                            t.id,
                            host_str,
                            t.direction,
                            t.status,
                            t.bytes_transferred,
                            t.total_bytes,
                            t.local_path,
                            t.remote_path
                        );
                    }
                }
                "clear-history" => {
                    let extra = &args[3..];
                    let host_id: Option<i64> = extract_flag(extra, "--host-id")
                        .map(|h| {
                            h.parse()
                                .map_err(|_| invalid("Host ID must be an integer."))
                        })
                        .transpose()?;
                    let count = store.clear_transfer_history(host_id).await?;
                    println!("Cleared {count} transfer record(s).");
                }
                "log" => {
                    let dir_str = &args[3];
                    let local_path = &args[4];
                    let remote_path = &args[5];
                    let bytes: u64 = args[6]
                        .parse()
                        .map_err(|_| invalid("Bytes transferred must be an integer."))?;
                    let extra = &args[7..];
                    let total_bytes: u64 = extract_flag(extra, "--total-bytes")
                        .map(|b| {
                            b.parse()
                                .map_err(|_| invalid("Total bytes must be an integer."))
                        })
                        .transpose()?
                        .unwrap_or(bytes);
                    let status =
                        extract_flag(extra, "--status").unwrap_or_else(|| "completed".to_string());
                    let host_id: Option<i64> = extract_flag(extra, "--host-id")
                        .map(|h| {
                            h.parse()
                                .map_err(|_| invalid("Host ID must be an integer."))
                        })
                        .transpose()?;

                    let direction = match dir_str.to_lowercase().as_str() {
                        "upload" => TransferDirection::Upload,
                        "download" => TransferDirection::Download,
                        _ => {
                            return Err(invalid("Direction must be 'upload' or 'download'.").into())
                        }
                    };

                    let tid = store
                        .log_transfer(&NewTransfer {
                            host_id,
                            direction,
                            local_path,
                            remote_path,
                            bytes_transferred: bytes,
                            total_bytes,
                            status: &status,
                            error_message: None,
                        })
                        .await?;
                    println!("{tid}");
                }
                _ => unreachable!(),
            }
        }
        "snippet" => {
            let sub = args[2].as_str();
            match sub {
                "list" => {
                    let extra = &args[3..];
                    let tag_filter = extract_flag(extra, "--tag");
                    let search_query = extract_flag(extra, "--search");

                    let snippets = if let Some(query) = search_query {
                        store.search_snippets(&query).await?
                    } else if let Some(tag) = tag_filter {
                        store.list_snippets_by_tag(&tag).await?
                    } else {
                        store.list_snippets().await?
                    };

                    for s in snippets {
                        let tags = if s.tags.is_empty() {
                            "-".to_string()
                        } else {
                            s.tags.join(",")
                        };
                        let var_count = s.variables.len();
                        let desc = s.description.as_deref().unwrap_or("-");
                        println!(
                            "{}\t{}\t{}\tvars:{}\t{}",
                            s.id, s.name, tags, var_count, desc
                        );
                    }
                }
                "add" => {
                    let name = &args[3];
                    let command = &args[4];
                    let extra = &args[5..];
                    let description = extract_flag(extra, "--description");
                    let tags: Vec<String> = extract_flag(extra, "--tags")
                        .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
                        .unwrap_or_default();

                    let snippet = Snippet::new(name, command, description, &tags)?;
                    let id = store.add_snippet(&snippet).await?;
                    println!("{id}");
                }
                "show" => {
                    let id: i64 = args[3].parse()?;
                    if let Some(s) = store.get_snippet(id).await? {
                        println!("ID:          {}", s.id);
                        println!("Name:        {}", s.name);
                        println!(
                            "Description: {}",
                            s.description.as_deref().unwrap_or("(none)")
                        );
                        println!(
                            "Tags:        {}",
                            if s.tags.is_empty() {
                                "(none)".to_string()
                            } else {
                                s.tags.join(", ")
                            }
                        );
                        println!("Command:     {}", s.command);
                        if s.variables.is_empty() {
                            println!("Variables:   (none)");
                        } else {
                            println!("Variables:");
                            for v in &s.variables {
                                let def = v.default_value.as_deref().unwrap_or("required");
                                let sec = if v.is_secret { " [SECRET]" } else { "" };
                                println!("  - {}{}: default={}", v.name, sec, def);
                            }
                        }
                    } else {
                        return Err(invalid("Snippet not found.").into());
                    }
                }
                "render" => {
                    let id: i64 = args[3].parse()?;
                    let snippet = store
                        .get_snippet(id)
                        .await?
                        .ok_or_else(|| invalid("Snippet not found."))?;

                    let mut params = HashMap::new();
                    let mut iter = args[4..].iter();
                    while let Some(arg) = iter.next() {
                        if arg == "--var" {
                            if let Some(pair) = iter.next() {
                                if let Some((k, v)) = pair.split_once('=') {
                                    params.insert(k.trim().to_string(), v.trim().to_string());
                                }
                            }
                        }
                    }

                    let preview_only = args[4..].iter().any(|a| a == "--preview");
                    let rendered = if preview_only {
                        render_preview(&snippet.command, &params, &snippet.variables)?
                    } else {
                        render_command(&snippet.command, &params)?
                    };
                    println!("{rendered}");
                }
                "delete" => {
                    let id: i64 = args[3].parse()?;
                    if !store.delete_snippet(id).await? {
                        return Err(invalid("Snippet not found.").into());
                    }
                    println!("Snippet {id} deleted.");
                }
                _ => unreachable!(),
            }
        }
        "vault" => {
            let action = args[2].as_str();
            match action {
                "export" => {
                    let output_path = &args[3];
                    let extra = &args[4..];
                    let passphrase = extract_flag(extra, "--passphrase")
                        .unwrap_or_else(|| "default-vault-passphrase".to_string());
                    let device_id = extract_flag(extra, "--device-id")
                        .unwrap_or_else(|| "shellumi-cli-device".to_string());
                    let iterations = extract_flag(extra, "--iterations")
                        .and_then(|i| i.parse::<u32>().ok())
                        .unwrap_or(shellumi_core::DEFAULT_KDF_ITERATIONS);

                    let vault = store
                        .export_vault_with_iterations(&passphrase, &device_id, iterations)
                        .await?;
                    let json = vault.to_json()?;
                    fs::write(output_path, json)?;
                    println!(
                        "Vault exported successfully to {output_path} ({} records).",
                        vault.records.len()
                    );
                }
                "import" => {
                    let input_path = &args[3];
                    let extra = &args[4..];
                    let passphrase = extract_flag(extra, "--passphrase")
                        .unwrap_or_else(|| "default-vault-passphrase".to_string());

                    let json = fs::read_to_string(input_path)?;
                    let vault = SyncVault::from_json(&json)?;
                    let summary = store.import_vault(&passphrase, &vault).await?;
                    println!("Vault imported from {input_path}:");
                    println!("  Hosts imported:    {}", summary.hosts_imported);
                    println!("  Groups imported:   {}", summary.groups_imported);
                    println!("  Tunnels imported:  {}", summary.tunnels_imported);
                    println!("  Snippets imported: {}", summary.snippets_imported);
                    if !summary.errors.is_empty() {
                        eprintln!("  Errors ({}):", summary.errors.len());
                        for err in summary.errors {
                            eprintln!("    - {err}");
                        }
                    }
                }
                "status" => {
                    let hosts = store.list().await?.len();
                    let groups = store.list_groups().await?.len();
                    let tunnels = store.list_tunnels().await?.len();
                    let snippets = store.list_snippets().await?.len();
                    println!("Local Vault Status:");
                    println!("  Hosts:    {hosts}");
                    println!("  Groups:   {groups}");
                    println!("  Tunnels:  {tunnels}");
                    println!("  Snippets: {snippets}");
                }
                "diff" => {
                    let input_path = &args[3];
                    let extra = &args[4..];
                    let passphrase = extract_flag(extra, "--passphrase")
                        .unwrap_or_else(|| "default-vault-passphrase".to_string());
                    let device_id = extract_flag(extra, "--device-id")
                        .unwrap_or_else(|| "shellumi-cli-device".to_string());

                    let json = fs::read_to_string(input_path)?;
                    let remote_vault = SyncVault::from_json(&json)?;
                    let local_vault = store
                        .export_vault_with_iterations(&passphrase, &device_id, 2000)
                        .await?;

                    let diffs = diff_sync_records_with_passphrase(
                        &local_vault.records,
                        &remote_vault.records,
                        Some(&passphrase),
                    );
                    if diffs.is_empty() {
                        println!("No records found in either vault.");
                    } else {
                        println!("OBJECT ID\tTYPE\tSTATUS\tLOCAL VER\tREMOTE VER");
                        for d in diffs {
                            let loc_v = d
                                .local_version
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "-".into());
                            let rem_v = d
                                .remote_version
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "-".into());
                            println!(
                                "{}\t{}\t{}\t{}\t{}",
                                d.object_id, d.entity_type, d.status, loc_v, rem_v
                            );
                        }
                    }
                }
                "oauth-url" => {
                    let extra = &args[3..];
                    let client_id = extract_flag(extra, "--client-id").unwrap_or_else(|| {
                        "shellumi-client-id.apps.googleusercontent.com".to_string()
                    });
                    let redirect_uri = extract_flag(extra, "--redirect-uri")
                        .unwrap_or_else(|| "http://127.0.0.1:8080/callback".to_string());

                    let pkce = generate_pkce_challenge()?;
                    let state = format!(
                        "state_{}",
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs()
                    );
                    let auth_url =
                        build_google_drive_auth_url(&client_id, &redirect_uri, &pkce, &state);

                    println!("OAuth 2.0 PKCE Challenge Generated:");
                    println!("  Code Verifier:  {}", pkce.code_verifier);
                    println!("  Code Challenge: {}", pkce.code_challenge);
                    println!("  State Nonce:    {}", state);
                    println!("\nAuthorization URL:\n{}", auth_url);
                }
                "history" => {
                    let history = store.list_snapshot_history().await?;
                    if history.is_empty() {
                        println!("No sync snapshots archived in database.");
                    } else {
                        println!("ID\tSNAPSHOT ID\tRECORDS\tCREATED AT");
                        for entry in history {
                            println!(
                                "{}\t{}\t{}\t{}",
                                entry.id, entry.snapshot_id, entry.record_count, entry.created_at
                            );
                        }
                    }
                }
                "rollback" => {
                    let snapshot_id = &args[3];
                    let extra = &args[4..];
                    let passphrase = extract_flag(extra, "--passphrase")
                        .unwrap_or_else(|| "default-vault-passphrase".to_string());

                    let summary = store.rollback_snapshot(snapshot_id, &passphrase).await?;
                    println!("Rolled back to snapshot {snapshot_id}:");
                    println!("  Hosts imported:    {}", summary.hosts_imported);
                    println!("  Groups imported:   {}", summary.groups_imported);
                    println!("  Tunnels imported:  {}", summary.tunnels_imported);
                    println!("  Snippets imported: {}", summary.snippets_imported);
                    if !summary.errors.is_empty() {
                        eprintln!("  Errors ({}):", summary.errors.len());
                        for err in summary.errors {
                            eprintln!("    - {err}");
                        }
                    }
                }
                _ => unreachable!(),
            }
        }
        "quick-connect" => {
            let destination = parse_quick_connect_target(&args[2])?;
            let status = Command::new("ssh")
                .args(destination.openssh_arguments())
                .status()?;
            if !status.success() {
                return Err(io::Error::other(format!("OpenSSH exited with {status}")).into());
            }
        }
        "import-ssh-config" => {
            let content = fs::read_to_string(&args[2])?;
            let entries = parse_ssh_config(&content, None);
            let mut count = 0;
            for entry in &entries {
                store
                    .add(&entry.name, &entry.destination, None, &[], None)
                    .await?;
                count += 1;
            }
            println!("Imported {count} host(s) from SSH config.");
        }
        "export-ssh-config" => {
            let hosts = store.list().await?;
            let entries: Vec<_> = hosts
                .into_iter()
                .map(|h| HostConfigEntry::new(h.name, h.destination))
                .collect();
            print!("{}", export_ssh_config(&entries));
        }
        "import-csv" => {
            let content = fs::read_to_string(&args[2])?;
            let entries = parse_csv(&content).map_err(|e| invalid(&e))?;
            let mut count = 0;
            for entry in &entries {
                store
                    .add(&entry.name, &entry.destination, None, &[], None)
                    .await?;
                count += 1;
            }
            println!("Imported {count} host(s) from CSV.");
        }
        "export-csv" => {
            let hosts = store.list().await?;
            let entries: Vec<_> = hosts
                .into_iter()
                .map(|h| HostConfigEntry::new(h.name, h.destination))
                .collect();
            print!("{}", export_csv(&entries));
        }
        _ => unreachable!(),
    }
    Ok(())
}

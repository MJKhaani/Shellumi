//! Out-of-process migration worker supervision & stdio IPC.
//!
//! Isolates legacy database migration (including Turso schema parsing and KDF)
//! in a dedicated child process (`shellumi-app --migration-worker --root <dir>`)
//! so native signals (such as SIGILL or OOM) cannot terminate the GPUI window
//! and are cleanly contained, reported, and recoverable via the migration journal.

use serde::{Deserialize, Serialize};
use shellumi_core::{VaultId, VaultKey, VaultKeyManager, VaultManifest};
use shellumi_storage::{
    HostStore, MigrationCredentials, MigrationEngine, MigrationProgressEvent, MigrationReport,
    LEGACY_LOCAL_VAULT_ID,
};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use zeroize::Zeroizing;

static WORKER_RUNNING: AtomicBool = AtomicBool::new(false);

#[derive(Serialize, Deserialize)]
pub(crate) struct WorkerRequest {
    pub(crate) default_password: String,
    pub(crate) default_recovery_code: String,
    pub(crate) source_password: Option<String>,
    pub(crate) remember_in_keychain: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", content = "payload")]
pub(crate) enum WorkerEvent {
    Progress(MigrationProgressEvent),
    Completed {
        report: MigrationReport,
        recovery_code_matches: bool,
    },
    Failed(String),
}

/// Entry point when `shellumi-app` is invoked as `--migration-worker --root <dir>`.
/// Returns `Some(exit_code)` if worker mode was requested, or `None` for normal GUI startup.
pub(crate) fn maybe_run_migration_worker() -> Option<i32> {
    let mut args = std::env::args_os().skip(1);
    let mut is_worker = false;
    let mut root: Option<PathBuf> = None;

    while let Some(arg) = args.next() {
        let s = arg.to_string_lossy();
        if s == "--migration-worker" {
            is_worker = true;
        } else if s == "--root" {
            if let Some(val) = args.next() {
                root = Some(PathBuf::from(val));
            }
        }
    }

    if !is_worker {
        return None;
    }

    let Some(base_dir) = root else {
        let _ = writeln!(
            std::io::stdout(),
            "{}",
            serde_json::to_string(&WorkerEvent::Failed(
                "Missing --root argument for migration worker".to_string()
            ))
            .unwrap_or_default()
        );
        return Some(2);
    };

    let mut stdin_buf = Zeroizing::new(String::new());
    if let Err(e) = std::io::stdin().read_to_string(&mut stdin_buf) {
        let _ = writeln!(
            std::io::stdout(),
            "{}",
            serde_json::to_string(&WorkerEvent::Failed(format!(
                "Failed to read worker IPC request: {e}"
            )))
            .unwrap_or_default()
        );
        return Some(2);
    }

    let req: WorkerRequest = match serde_json::from_str(&stdin_buf) {
        Ok(r) => r,
        Err(e) => {
            let _ = writeln!(
                std::io::stdout(),
                "{}",
                serde_json::to_string(&WorkerEvent::Failed(format!(
                    "Invalid worker IPC request: {e}"
                )))
                .unwrap_or_default()
            );
            return Some(2);
        }
    };

    let code = run_worker_sync(&base_dir, &req);
    Some(code)
}

fn emit_event(ev: &WorkerEvent) {
    if let Ok(line) = serde_json::to_string(ev) {
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{line}");
        let _ = out.flush();
    }
}

fn run_worker_sync(base_dir: &std::path::Path, req: &WorkerRequest) -> i32 {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            emit_event(&WorkerEvent::Failed(format!(
                "Worker runtime init failed: {e}"
            )));
            return 1;
        }
    };

    let res: Result<(MigrationReport, bool), String> = runtime.block_on(async {
        let cred_store = shellumi_core::default_credential_store();
        let vid = VaultId::new(LEGACY_LOCAL_VAULT_ID).map_err(|e| e.to_string())?;

        let source_key = if let Some(src_pwd) =
            req.source_password.as_deref().filter(|s| !s.is_empty())
        {
            let candidates = [
                base_dir.join("vault_manifests/legacy-local.manifest.json"),
                base_dir.join("vault_manifests/default.manifest.json"),
                base_dir.join("manifests/legacy-local.manifest.json"),
            ];
            if let Some(mpath) = candidates.into_iter().find(|p| p.exists()) {
                let manifest = VaultManifest::load_from_path(&mpath).map_err(|e| e.to_string())?;
                Some(
                    manifest
                        .unlock(src_pwd)
                        .map_err(|_| "Incorrect legacy database password.".to_string())?,
                )
            } else {
                VaultKeyManager::retrieve_key(cred_store.as_ref(), &vid)
                    .map_err(|e| e.to_string())?
            }
        } else {
            None
        };

        let creds = MigrationCredentials {
            source_key,
            default_password: Some(req.default_password.clone()),
            default_recovery_code: Some(req.default_recovery_code.clone()),
            secondary_passwords: Default::default(),
            secondary_recovery_codes: Default::default(),
            require_portable_manifests: true,
            remember_in_keychain: req.remember_in_keychain,
        };

        let report = MigrationEngine::migrate_legacy_database_with_progress(
            base_dir,
            None,
            Some(&creds),
            cred_store.as_ref(),
            &mut |ev| emit_event(&WorkerEvent::Progress(ev)),
        )
        .await
        .map_err(|e| format!("Migration failed: {e}"))?;

        // Verify activated Default Vault manifest (both password & recovery unwrap) and DB reopen.
        let mpath = base_dir.join("vault_manifests/legacy-local.manifest.json");
        let manifest = VaultManifest::load_from_path(&mpath)
            .map_err(|e| format!("Activated manifest verification failed: {e}"))?;
        let key_pwd =
            VaultKey::unwrap_with_password(&vid, &req.default_password, &manifest.primary_envelope)
                .map_err(|_| "Activated manifest password unwrap failed.".to_string())?;

        let recovery_matches = if let Some(rec_env) = &manifest.recovery_envelope {
            match VaultKey::recover(&vid, &req.default_recovery_code, rec_env) {
                Ok(key_rec) => key_rec.expose_bytes() == key_pwd.expose_bytes(),
                Err(_) => false,
            }
        } else {
            false
        };
        if !recovery_matches {
            return Err(
                "Activated manifest recovery code verification failed; the staged or activated recovery envelope does not match the displayed recovery code."
                    .to_string(),
            );
        }

        let db_path = base_dir.join("vaults/default.db");
        let db_str = db_path
            .to_str()
            .ok_or_else(|| "Invalid default.db path".to_string())?;
        let store = HostStore::open_existing_with_key(db_str, Some(&key_pwd))
            .await
            .map_err(|e| format!("Activated database reopen verification failed: {e}"))?;
        let _ = store
            .build_vault_directory()
            .await
            .map_err(|e| e.to_string())?;

        Ok((report, recovery_matches))
    });

    match res {
        Ok((report, recovery_code_matches)) => {
            emit_event(&WorkerEvent::Completed {
                report,
                recovery_code_matches,
            });
            0
        }
        Err(e) => {
            emit_event(&WorkerEvent::Failed(e));
            1
        }
    }
}

/// Supervises the out-of-process migration worker from the GPUI controller.
pub(crate) fn run_supervised_migration_worker(
    base_dir: &std::path::Path,
    req: &WorkerRequest,
    on_progress: &mut dyn FnMut(MigrationProgressEvent),
) -> Result<(MigrationReport, bool), String> {
    if WORKER_RUNNING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Err(
            "A migration worker process is already running; wait for it to finish.".to_string(),
        );
    }
    struct WorkerGuard;
    impl Drop for WorkerGuard {
        fn drop(&mut self) {
            WORKER_RUNNING.store(false, Ordering::Release);
        }
    }
    let _guard = WorkerGuard;

    let exe = std::env::current_exe()
        .map_err(|e| format!("Failed to locate Shellumi executable for migration worker: {e}"))?;

    let mut cmd = shellumi_core::process::background_command(&exe);
    cmd.arg("--migration-worker")
        .arg("--root")
        .arg(base_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn migration worker process: {e}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        let payload = Zeroizing::new(
            serde_json::to_string(req).map_err(|e| format!("Serialize worker request: {e}"))?,
        );
        stdin
            .write_all(payload.as_bytes())
            .map_err(|e| format!("Failed to write request to migration worker: {e}"))?;
    }

    // Drain stderr concurrently with bounded capacity to prevent pipe deadlock.
    let stderr_handle = child.stderr.take().map(|stderr| {
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stderr);
            let mut buf = String::new();
            let mut line = String::new();
            while buf.len() < 16 * 1024 {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        let remaining = (16 * 1024usize).saturating_sub(buf.len());
                        if line.len() <= remaining {
                            buf.push_str(&line);
                        } else {
                            break;
                        }
                    }
                }
            }
            buf
        })
    });

    let mut completed: Option<(MigrationReport, bool)> = None;
    let mut failure_msg: Option<String> = None;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(600);
    let mut timed_out = false;

    if let Some(stdout) = child.stdout.take() {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        loop {
            if std::time::Instant::now() >= deadline {
                timed_out = true;
                let _ = child.kill();
                break;
            }
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    if line.len() > 64 * 1024 {
                        failure_msg =
                            Some("Migration worker emitted oversized IPC message.".into());
                        let _ = child.kill();
                        break;
                    }
                    if let Ok(ev) = serde_json::from_str::<WorkerEvent>(&line) {
                        match ev {
                            WorkerEvent::Progress(p) => on_progress(p),
                            WorkerEvent::Completed {
                                report,
                                recovery_code_matches,
                            } => {
                                completed = Some((report, recovery_code_matches));
                            }
                            WorkerEvent::Failed(msg) => {
                                failure_msg = Some(msg);
                            }
                        }
                    }
                }
            }
        }
    }

    let status = child
        .wait()
        .map_err(|e| format!("Failed waiting on migration worker: {e}"))?;
    let _stderr_out = stderr_handle
        .and_then(|h| h.join().ok())
        .unwrap_or_default();

    if timed_out {
        return Err(
            "Migration worker timed out and was terminated; your original database and migration journal were preserved."
                .to_string(),
        );
    }
    if let Some(msg) = failure_msg {
        return Err(msg);
    }
    if !status.success() {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            if let Some(sig) = status.signal() {
                return Err(format!(
                    "Migration worker terminated abnormally by OS signal {sig}; your original database and migration journal were preserved."
                ));
            }
        }
        return Err(format!(
            "Migration worker exited with status {status}; your original database and migration journal were preserved."
        ));
    }

    completed.ok_or_else(|| {
        "Migration worker exited without a completion event; check migration recovery status."
            .to_string()
    })
}

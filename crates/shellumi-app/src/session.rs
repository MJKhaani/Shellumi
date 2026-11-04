//! Runs a PTY in worker threads and streams bounded output to the GPUI task.
use shellumi_terminal::{PtySession, TerminalCommand};
use std::{io::Read, sync::mpsc, thread};

pub enum SessionEvent {
    Output(Vec<u8>),
    Error(String),
    Closed,
}

enum SessionCommand {
    Input(Vec<u8>),
    #[allow(dead_code)]
    Resize(u16, u16),
    Close,
}

pub struct Session {
    commands: mpsc::Sender<SessionCommand>,
    pub events: async_channel::Receiver<SessionEvent>,
}

impl Session {
    pub fn start(command: TerminalCommand) -> Self {
        let (commands, incoming) = mpsc::channel();
        let (outgoing, events) = async_channel::bounded(32);
        thread::spawn(move || {
            let mut pty = match PtySession::spawn(&command, 24, 80) {
                Ok(pty) => pty,
                Err(error) => {
                    let _ = outgoing.send_blocking(SessionEvent::Error(error.to_string()));
                    return;
                }
            };
            let mut reader = match pty.try_clone_reader() {
                Ok(reader) => reader,
                Err(error) => {
                    let _ = outgoing.send_blocking(SessionEvent::Error(error.to_string()));
                    return;
                }
            };
            let reader_events = outgoing.clone();
            thread::spawn(move || {
                let mut buffer = [0; 8192];
                loop {
                    match reader.read(&mut buffer) {
                        Ok(0) => break,
                        Ok(count) => {
                            if reader_events
                                .send_blocking(SessionEvent::Output(buffer[..count].to_vec()))
                                .is_err()
                            {
                                return;
                            }
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                        Err(_) => break,
                    }
                }
                let _ = reader_events.send_blocking(SessionEvent::Closed);
            });
            while let Ok(command) = incoming.recv() {
                match command {
                    SessionCommand::Input(bytes) => {
                        if let Err(error) = pty.write_all(&bytes) {
                            let _ = outgoing.send_blocking(SessionEvent::Error(error.to_string()));
                            break;
                        }
                    }
                    SessionCommand::Resize(rows, cols) => {
                        if let Err(error) = pty.resize(rows, cols) {
                            let _ = outgoing.send_blocking(SessionEvent::Error(error.to_string()));
                        }
                    }
                    SessionCommand::Close => break,
                }
            }
        });
        Self { commands, events }
    }

    pub fn input(&self, bytes: Vec<u8>) -> Result<(), &'static str> {
        self.commands
            .send(SessionCommand::Input(bytes))
            .map_err(|_| "Session is closed.")
    }

    #[allow(dead_code)]
    pub fn resize(&self, rows: u16, cols: u16) -> Result<(), &'static str> {
        self.commands
            .send(SessionCommand::Resize(rows, cols))
            .map_err(|_| "Session is closed.")
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.commands.send(SessionCommand::Close);
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn streams_output_from_a_real_pty() {
        let session = Session::start(TerminalCommand::new("/bin/echo").arg("session output"));
        let mut output = Vec::new();
        loop {
            match session.events.recv_blocking().unwrap() {
                SessionEvent::Output(bytes) => output.extend(bytes),
                SessionEvent::Closed => break,
                SessionEvent::Error(error) => panic!("{error}"),
            }
        }
        assert!(String::from_utf8_lossy(&output).contains("session output"));
    }

    #[test]
    fn sends_input_to_a_real_pty_and_receives_response() {
        let session = Session::start(
            TerminalCommand::new("/bin/sh")
                .args(["-c", "read line; printf 'response:%s\\n' \"$line\""]),
        );
        session.input(b"hello\r".to_vec()).unwrap();
        let mut output = Vec::new();
        loop {
            match session.events.recv_blocking().unwrap() {
                SessionEvent::Output(bytes) => output.extend(bytes),
                SessionEvent::Closed => break,
                SessionEvent::Error(error) => panic!("{error}"),
            }
        }
        assert!(String::from_utf8_lossy(&output).contains("response:hello"));
    }

    #[test]
    fn resizes_session_without_error() {
        let session = Session::start(TerminalCommand::new("/bin/echo").arg("resizing"));
        assert!(session.resize(30, 100).is_ok());
    }

    /// Opt in only with an isolated sshd configured to run `echo shellumi-auth-ok`.
    #[test]
    #[ignore = "requires an isolated loopback OpenSSH server and ephemeral test keys"]
    fn isolated_openssh_host_key_and_authentication() {
        use shellumi_core::SshDestination;
        use shellumi_terminal::Terminal;
        use std::{
            fs,
            time::{Duration, Instant},
        };

        let fixture = std::env::var("SHELLUMI_SSH_TEST_DIR")
            .expect("set SHELLUMI_SSH_TEST_DIR to the isolated SSH fixture directory");
        let port: u16 = std::env::var("SHELLUMI_SSH_TEST_PORT")
            .unwrap()
            .parse()
            .unwrap();
        let username = std::env::var("USER").unwrap();
        let destination = SshDestination::new("127.0.0.1", &username, port).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let known = dir.path().join("known_hosts");
        let key = std::path::Path::new(&fixture).join("client_key");

        let run = |answer: Option<&str>| {
            let command = TerminalCommand::new("/usr/bin/ssh")
                .args([
                    "-F",
                    "/dev/null",
                    "-i",
                    key.to_str().unwrap(),
                    "-o",
                    "IdentitiesOnly=yes",
                    "-o",
                    "GlobalKnownHostsFile=/dev/null",
                    "-o",
                    "ConnectTimeout=5",
                ])
                .arg("-o")
                .arg(format!("UserKnownHostsFile={}", known.display()))
                .args(destination.openssh_arguments());
            let session = Session::start(command);
            let deadline = Instant::now() + Duration::from_secs(15);
            let mut output = Vec::new();
            let mut screen = Terminal::new(24, 80);
            let mut prompted = false;
            loop {
                match session.events.try_recv() {
                    Ok(SessionEvent::Output(bytes)) => {
                        screen.process_bytes(&bytes);
                        output.extend(bytes);
                        if !prompted
                            && String::from_utf8_lossy(&output)
                                .contains("Are you sure you want to continue connecting")
                        {
                            prompted = true;
                            assert!(
                                screen.screen_to_string().contains("Are you sure"),
                                "host-key prompt was not rendered in the terminal grid"
                            );
                            if let Some(answer) = answer {
                                session.input(format!("{answer}\r").into_bytes()).unwrap();
                            }
                        }
                    }
                    Ok(SessionEvent::Closed) => break,
                    Ok(SessionEvent::Error(error)) => panic!("session failed: {error}"),
                    Err(async_channel::TryRecvError::Empty) => {
                        assert!(
                            Instant::now() < deadline,
                            "SSH session timed out: {}",
                            String::from_utf8_lossy(&output)
                        );
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    Err(async_channel::TryRecvError::Closed) => {
                        panic!("session channel closed unexpectedly")
                    }
                }
            }
            (prompted, String::from_utf8_lossy(&output).into_owned())
        };

        let (prompted, rejected) = run(Some("no"));
        assert!(prompted);
        assert!(!rejected.contains("shellumi-auth-ok"));
        assert!(!known.exists() || fs::read_to_string(&known).unwrap().trim().is_empty());

        let (prompted, accepted) = run(Some("yes"));
        assert!(prompted);
        assert!(
            accepted.contains("shellumi-auth-ok"),
            "authentication failed: {accepted}"
        );
        assert!(known.exists());

        let (prompted, known_result) = run(None);
        assert!(!prompted);
        assert!(
            known_result.contains("shellumi-auth-ok"),
            "known-host connection failed: {known_result}"
        );

        let wrong_key = fs::read_to_string(key.with_extension("pub")).unwrap();
        fs::write(&known, format!("[127.0.0.1]:{port} {wrong_key}")).unwrap();
        let (_, changed) = run(None);
        assert!(
            changed.contains("REMOTE HOST IDENTIFICATION HAS CHANGED"),
            "changed host key was not reported: {changed}"
        );
        assert!(!changed.contains("shellumi-auth-ok"));
    }
}

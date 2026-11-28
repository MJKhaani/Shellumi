//! Runs a PTY in worker threads and streams bounded output to the GPUI task.
use shellumi_terminal::{PtySession, TerminalCommand};
use std::{
    io::Read,
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc, Arc,
    },
    thread,
};

pub const MAX_INPUT_QUEUE_BYTES: usize = 1024 * 1024; // 1 MiB byte budget
pub const MAX_PASTE_BYTES: usize = 512 * 1024; // 512 KiB max single paste

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionInputError {
    QueueFull,
    Closed,
    PayloadTooLarge,
}

impl std::fmt::Display for SessionInputError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::QueueFull => write!(f, "Session input queue full (backpressure)."),
            Self::Closed => write!(f, "Session is closed."),
            Self::PayloadTooLarge => write!(f, "Input payload exceeds maximum allowed size."),
        }
    }
}

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
    commands: mpsc::SyncSender<SessionCommand>,
    queued_bytes: Arc<AtomicUsize>,
    pub events: async_channel::Receiver<SessionEvent>,
}

impl Session {
    pub fn start(command: TerminalCommand) -> Self {
        let (commands, incoming) = mpsc::sync_channel(256);
        let queued_bytes = Arc::new(AtomicUsize::new(0));
        let worker_queued_bytes = Arc::clone(&queued_bytes);
        let (outgoing, events) = async_channel::bounded(64);
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
                        let len = bytes.len();
                        worker_queued_bytes.fetch_sub(
                            len.min(worker_queued_bytes.load(Ordering::Relaxed)),
                            Ordering::Relaxed,
                        );
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
        Self {
            commands,
            queued_bytes,
            events,
        }
    }

    pub fn input(&self, bytes: Vec<u8>) -> Result<(), SessionInputError> {
        let len = bytes.len();
        if len > MAX_PASTE_BYTES {
            return Err(SessionInputError::PayloadTooLarge);
        }
        let cur = self.queued_bytes.load(Ordering::Relaxed);
        if cur.saturating_add(len) > MAX_INPUT_QUEUE_BYTES {
            return Err(SessionInputError::QueueFull);
        }
        self.queued_bytes.fetch_add(len, Ordering::Relaxed);
        match self.commands.try_send(SessionCommand::Input(bytes)) {
            Ok(()) => Ok(()),
            Err(mpsc::TrySendError::Full(_)) => {
                self.queued_bytes.fetch_sub(len, Ordering::Relaxed);
                Err(SessionInputError::QueueFull)
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                self.queued_bytes.fetch_sub(len, Ordering::Relaxed);
                Err(SessionInputError::Closed)
            }
        }
    }

    #[allow(dead_code)]
    pub fn resize(&self, rows: u16, cols: u16) -> Result<(), SessionInputError> {
        self.commands
            .try_send(SessionCommand::Resize(rows, cols))
            .map_err(|e| match e {
                mpsc::TrySendError::Full(_) => SessionInputError::QueueFull,
                mpsc::TrySendError::Disconnected(_) => SessionInputError::Closed,
            })
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        let _ = self.commands.try_send(SessionCommand::Close);
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

    #[test]
    fn rejects_oversized_payload_without_closing_session() {
        let session = Session::start(TerminalCommand::new("/bin/echo").arg("ok"));
        let err = session.input(vec![b'a'; MAX_PASTE_BYTES + 1]).unwrap_err();
        assert_eq!(err, SessionInputError::PayloadTooLarge);
    }
}

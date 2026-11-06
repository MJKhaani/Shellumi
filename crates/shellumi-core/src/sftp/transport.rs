//! Bounded SSH pipe IO. Worker threads own the blocking OS handles; callers wait
//! on bounded channels with one deadline per complete packet and cancellation polls.
use super::TransferControl;
use std::io::{self, Cursor, Read, Write};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender};
use std::time::{Duration, Instant};

struct Budget {
    timeout: Duration,
    deadline: Instant,
    control: Option<TransferControl>,
}
impl Budget {
    fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            deadline: Instant::now() + timeout,
            control: None,
        }
    }
    fn begin(&mut self, control: Option<&TransferControl>) {
        self.deadline = Instant::now() + self.timeout;
        self.control = control.cloned();
    }
    fn remaining(&self) -> io::Result<Duration> {
        if self
            .control
            .as_ref()
            .is_some_and(TransferControl::is_cancelled)
        {
            return Err(io::Error::new(
                io::ErrorKind::ConnectionAborted,
                "SSH IO cancelled",
            ));
        }
        let remaining = self.deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "SSH packet IO timed out",
            ));
        }
        Ok(remaining.min(Duration::from_millis(25)))
    }
    fn receive<T>(&self, receiver: &Receiver<T>) -> io::Result<T> {
        loop {
            match receiver.recv_timeout(self.remaining()?) {
                Ok(value) => return Ok(value),
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "SSH pipe closed",
                    ))
                }
            }
        }
    }
}
pub(super) enum PacketReader {
    Direct(Box<dyn Read + Send>),
    Process(BoundedReader),
}
impl PacketReader {
    pub fn process(source: impl Read + Send + 'static, timeout: Duration) -> io::Result<Self> {
        Ok(Self::Process(BoundedReader::new(source, timeout)?))
    }
    pub fn begin(&mut self, control: Option<&TransferControl>) {
        if let Self::Process(reader) = self {
            reader.budget.begin(control);
        }
    }
}
impl Read for PacketReader {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Direct(reader) => reader.read(bytes),
            Self::Process(reader) => reader.read(bytes),
        }
    }
}
pub(super) struct BoundedReader {
    receiver: Receiver<io::Result<Vec<u8>>>,
    buffer: Cursor<Vec<u8>>,
    budget: Budget,
    failed: bool,
}
impl BoundedReader {
    fn new(mut source: impl Read + Send + 'static, timeout: Duration) -> io::Result<Self> {
        let (sender, receiver) = mpsc::sync_channel(2);
        std::thread::Builder::new()
            .name("shellumi-ssh-read".into())
            .spawn(move || loop {
                let mut buffer = vec![0u8; 32768];
                match source.read(&mut buffer) {
                    Ok(size) => {
                        buffer.truncate(size);
                        if sender.send(Ok(buffer)).is_err() || size == 0 {
                            break;
                        }
                    }
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error) => {
                        let _ = sender.send(Err(error));
                        break;
                    }
                }
            })?;
        Ok(Self {
            receiver,
            buffer: Cursor::new(Vec::new()),
            budget: Budget::new(timeout),
            failed: false,
        })
    }
}
impl Read for BoundedReader {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        if self.failed {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "SSH reader is no longer usable",
            ));
        }
        let result = (|| {
            self.budget.remaining()?;
            if self.buffer.position() < self.buffer.get_ref().len() as u64 {
                return self.buffer.read(bytes);
            }
            let buffer = self.budget.receive(&self.receiver)??;
            self.buffer = Cursor::new(buffer);
            self.buffer.read(bytes)
        })();
        if result.is_err() {
            self.failed = true;
        }
        result
    }
}
pub(super) enum PacketWriter {
    Direct(Box<dyn Write + Send>),
    Process(BoundedWriter),
}
impl PacketWriter {
    pub fn process(sink: impl Write + Send + 'static, timeout: Duration) -> io::Result<Self> {
        Ok(Self::Process(BoundedWriter::new(sink, timeout)?))
    }
    pub fn begin(&mut self, control: Option<&TransferControl>) {
        if let Self::Process(writer) = self {
            writer.budget.begin(control);
        }
    }
}
impl Write for PacketWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        match self {
            Self::Direct(writer) => writer.write(bytes),
            Self::Process(writer) => writer.write(bytes),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Direct(writer) => writer.flush(),
            Self::Process(_) => Ok(()),
        }
    }
}
pub(super) struct BoundedWriter {
    sender: SyncSender<Vec<u8>>,
    receiver: Receiver<io::Result<()>>,
    budget: Budget,
    failed: bool,
}
impl BoundedWriter {
    fn new(mut sink: impl Write + Send + 'static, timeout: Duration) -> io::Result<Self> {
        let (sender, packets) = mpsc::sync_channel::<Vec<u8>>(1);
        let (results, receiver) = mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("shellumi-ssh-write".into())
            .spawn(move || {
                while let Ok(packet) = packets.recv() {
                    let result = sink.write_all(&packet).and_then(|_| sink.flush());
                    let failed = result.is_err();
                    if results.send(result).is_err() || failed {
                        break;
                    }
                }
            })?;
        Ok(Self {
            sender,
            receiver,
            budget: Budget::new(timeout),
            failed: false,
        })
    }
}
impl Write for BoundedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.failed {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "SSH writer is no longer usable",
            ));
        }
        let result = (|| {
            self.budget.remaining()?;
            self.sender
                .try_send(bytes.to_vec())
                .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "SSH writer unavailable"))?;
            self.budget.receive(&self.receiver)??;
            Ok(bytes.len())
        })();
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Blocked(Receiver<()>);
    impl Read for Blocked {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            let _ = self.0.recv();
            Ok(0)
        }
    }
    impl Write for Blocked {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            let _ = self.0.recv();
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    fn cancellation(cancel: bool) -> TransferControl {
        let token = TransferControl::default();
        if cancel {
            let other = token.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(30));
                other.cancel();
            });
        }
        token
    }
    #[test]
    fn blocked_reads_timeout_or_cancel_without_waiting_for_the_os_pipe() {
        for cancel in [false, true] {
            let (release, blocked) = mpsc::channel();
            let mut reader =
                BoundedReader::new(Blocked(blocked), Duration::from_millis(100)).unwrap();
            let token = cancellation(cancel);
            reader.budget.begin(Some(&token));
            let start = Instant::now();
            let error = reader.read_exact(&mut [0u8; 1]).unwrap_err();
            assert_eq!(
                error.kind(),
                if cancel {
                    io::ErrorKind::ConnectionAborted
                } else {
                    io::ErrorKind::TimedOut
                }
            );
            assert!(start.elapsed() < Duration::from_secs(2));
            drop(release);
        }
    }
    #[test]
    fn blocked_writes_timeout_or_cancel_and_cannot_be_reused() {
        for cancel in [false, true] {
            let (release, blocked) = mpsc::channel();
            let mut writer =
                BoundedWriter::new(Blocked(blocked), Duration::from_millis(100)).unwrap();
            let token = cancellation(cancel);
            writer.budget.begin(Some(&token));
            let start = Instant::now();
            let error = writer.write_all(b"packet").unwrap_err();
            assert_eq!(
                error.kind(),
                if cancel {
                    io::ErrorKind::ConnectionAborted
                } else {
                    io::ErrorKind::TimedOut
                }
            );
            assert!(start.elapsed() < Duration::from_secs(2));
            assert_eq!(
                writer.write_all(b"retry").unwrap_err().kind(),
                io::ErrorKind::BrokenPipe
            );
            drop(release);
        }
    }
    #[test]
    fn buffered_reads_preserve_bytes_across_packet_boundaries() {
        let mut reader =
            PacketReader::process(Cursor::new(b"abcdefg".to_vec()), Duration::from_secs(1))
                .unwrap();
        reader.begin(None);
        let mut first = [0u8; 3];
        reader.read_exact(&mut first).unwrap();
        reader.begin(None);
        let mut second = [0u8; 4];
        reader.read_exact(&mut second).unwrap();
        assert_eq!(&first, b"abc");
        assert_eq!(&second, b"defg");
    }
    #[test]
    fn trickled_bytes_do_not_reset_the_whole_packet_deadline() {
        struct Slow;
        impl Read for Slow {
            fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
                std::thread::sleep(Duration::from_millis(20));
                bytes[0] = 1;
                Ok(1)
            }
        }
        let mut reader = BoundedReader::new(Slow, Duration::from_millis(70)).unwrap();
        reader.budget.begin(None);
        let start = Instant::now();
        assert_eq!(
            reader.read_exact(&mut [0u8; 30]).unwrap_err().kind(),
            io::ErrorKind::TimedOut
        );
        assert!(start.elapsed() < Duration::from_secs(2));
    }
}

#![forbid(unsafe_code)]
use crate::ffi::Frame;
use socket2::{Domain, SockAddr, Socket, Type};
use std::{
    io::{self, Read, Write},
    net::Shutdown,
    os::unix::net::UnixStream,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::Duration,
};

const MAX_FRAME_BYTES: usize = 128 * 1024 * 1024;
const IO_TIMEOUT: Duration = Duration::from_millis(250);

#[derive(Default)]
struct Shared {
    cancelled: AtomicBool,
    viewport: AtomicU64,
    socket: Mutex<Option<UnixStream>>,
    pending: Mutex<Frame>,
}
impl Shared {
    fn stop(&self) {
        self.cancelled.store(true, Ordering::Release);
        if let Some(socket) = self.socket.lock().unwrap().take() {
            let _ = socket.shutdown(Shutdown::Both);
        }
    }
    fn publish(&self, frame: Frame) {
        // A slow/hidden UI only retains the newest frame, never a frame queue.
        *self.pending.lock().unwrap() = frame;
    }
}

#[derive(Default)]
pub struct VideoReceiver {
    shared: Arc<Shared>,
    worker: Option<thread::JoinHandle<()>>,
}
impl VideoReceiver {
    fn stop(&mut self) {
        self.shared.stop();
        if let Some(worker) = self.worker.take() {
            worker.thread().unpark();
            // No Rust worker can outlive its QML item / loaded plugin code.
            // Shutdown wakes reads; even an unconnected socket has a deadline.
            let _ = worker.join();
        }
    }
    pub fn start(&mut self, path: &str, participant: &str, source: &str) {
        self.stop();
        self.shared = Arc::new(Shared::default());
        if path.is_empty() || participant.is_empty() || source.is_empty() {
            return;
        }
        let path = path.to_owned();
        let handshake =
            serde_json::json!({"participant":participant,"source":source}).to_string() + "\n";
        let shared = self.shared.clone();
        let result = thread::Builder::new()
            .name("wisp-video".into())
            .spawn(move || {
                let result = (|| {
                    let socket = Socket::new(Domain::UNIX, Type::STREAM, None)?;
                    socket.connect_timeout(&SockAddr::unix(path)?, IO_TIMEOUT)?;
                    let fd: std::os::fd::OwnedFd = socket.into();
                    let mut socket = UnixStream::from(fd);
                    socket.set_read_timeout(Some(IO_TIMEOUT))?;
                    socket.set_write_timeout(Some(IO_TIMEOUT))?;
                    *shared.socket.lock().unwrap() = Some(socket.try_clone()?);
                    if shared.cancelled.load(Ordering::Acquire) {
                        return Ok(());
                    }
                    socket.write_all(handshake.as_bytes())?;
                    receive(&mut socket, &shared)
                })();
                if let Err(error) = result
                    && !shared.cancelled.load(Ordering::Acquire)
                {
                    let message = if error.kind() == io::ErrorKind::InvalidData {
                        "Invalid stream dimensions"
                    } else {
                        "Stream connection lost"
                    };
                    shared.publish(Frame {
                        changed: true,
                        error: message.into(),
                        ..Frame::default()
                    });
                }
                shared.socket.lock().unwrap().take();
            });
        match result {
            Ok(worker) => self.worker = Some(worker),
            Err(_) => self.shared.publish(Frame {
                changed: true,
                error: "Could not start stream receiver".into(),
                ..Frame::default()
            }),
        }
    }
    pub fn viewport(&self, width: u32, height: u32) {
        let width = width.clamp(1, 16384);
        let height = height.clamp(1, 16384);
        self.shared.viewport.store(
            (u64::from(width) << 32) | u64::from(height),
            Ordering::Relaxed,
        );
    }
    pub fn poll(&self) -> Frame {
        std::mem::take(&mut self.shared.pending.lock().unwrap())
    }
}
impl Drop for VideoReceiver {
    fn drop(&mut self) {
        self.stop();
    }
}

fn frame_size(header: [u8; 8]) -> io::Result<(u32, u32, usize)> {
    let width = u32::from_le_bytes(header[..4].try_into().unwrap());
    let height = u32::from_le_bytes(header[4..].try_into().unwrap());
    if width == 0 || height == 0 || width > 16384 || height > 16384 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid frame dimensions",
        ));
    }
    let bytes = u64::from(width) * u64::from(height) * 4;
    if bytes > MAX_FRAME_BYTES as u64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid frame dimensions",
        ));
    }
    Ok((width, height, bytes as usize))
}

fn read_exact(socket: &mut UnixStream, mut bytes: &mut [u8], shared: &Shared) -> io::Result<()> {
    while !bytes.is_empty() {
        if shared.cancelled.load(Ordering::Acquire) {
            return Err(io::ErrorKind::Interrupted.into());
        }
        match socket.read(bytes) {
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(count) => bytes = &mut bytes[count..],
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::Interrupted
                ) => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

fn receive(socket: &mut UnixStream, shared: &Shared) -> io::Result<()> {
    while !shared.cancelled.load(Ordering::Acquire) {
        let size = shared.viewport.load(Ordering::Relaxed);
        writeln!(socket, "{} {}", (size >> 32).max(1), (size as u32).max(1))?;
        let mut header = [0_u8; 8];
        read_exact(socket, &mut header, shared)?;
        let (width, height, count) = frame_size(header)?;
        let mut pixels = vec![0; count];
        read_exact(socket, &mut pixels, shared)?;
        shared.publish(Frame {
            changed: true,
            width,
            height,
            pixels,
            error: String::new(),
        });
        thread::park_timeout(Duration::from_millis(16));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn real_socket_handshake_frame_and_drop_close_the_worker() {
        use std::{
            io::{BufRead, BufReader},
            os::unix::net::UnixListener,
            sync::mpsc,
        };
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("fixture.video");
        let listener = UnixListener::bind(&path).unwrap();
        let (sent, received) = mpsc::channel();
        let server = thread::spawn(move || {
            let (socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut reader = BufReader::new(socket);
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            let value: serde_json::Value = serde_json::from_str(&line).unwrap();
            assert_eq!(value["participant"], "Friend \"fixture\"");
            assert_eq!(value["source"], "camera");
            line.clear();
            reader.read_line(&mut line).unwrap();
            reader
                .get_mut()
                .write_all(&[2, 0, 0, 0, 1, 0, 0, 0, 255, 0, 0, 255, 0, 255, 0, 255])
                .unwrap();
            sent.send(()).unwrap();
            // Drain the next viewport request, then wait for Drop's shutdown.
            loop {
                line.clear();
                if reader.read_line(&mut line).unwrap() == 0 {
                    break;
                }
            }
        });
        let mut receiver = VideoReceiver::default();
        receiver.start(path.to_str().unwrap(), "Friend \"fixture\"", "camera");
        receiver.viewport(1920, 800);
        received.recv_timeout(Duration::from_secs(2)).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        loop {
            let frame = receiver.poll();
            if frame.changed {
                assert_eq!((frame.width, frame.height, frame.pixels.len()), (2, 1, 8));
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            thread::sleep(Duration::from_millis(1));
        }
        drop(receiver);
        server.join().unwrap();
    }
    #[test]
    fn dimensions_are_bounded_before_allocation() {
        for (w, h) in [
            (0_u32, 1_u32),
            (16385, 1),
            (8192, 8192),
            (u32::MAX, u32::MAX),
        ] {
            let mut header = [0; 8];
            header[..4].copy_from_slice(&w.to_le_bytes());
            header[4..].copy_from_slice(&h.to_le_bytes());
            assert!(frame_size(header).is_err());
        }
    }
    #[test]
    fn pending_frames_replace_instead_of_queue_and_source_reset_discards_them() {
        let mut receiver = VideoReceiver::default();
        for byte in 0..20 {
            receiver.shared.publish(Frame {
                changed: true,
                width: 1,
                height: 1,
                pixels: vec![byte; 4],
                error: String::new(),
            });
        }
        assert_eq!(receiver.poll().pixels, vec![19; 4]);
        assert!(!receiver.poll().changed);
        receiver.shared.publish(Frame {
            changed: true,
            ..Frame::default()
        });
        let previous = receiver.shared.clone();
        receiver.start("", "", "");
        assert!(previous.cancelled.load(Ordering::Acquire));
        assert!(!receiver.poll().changed);
    }
    #[test]
    fn fragmented_reads_and_cancellation_while_publisher_is_paused() {
        let (mut read, mut write) = UnixStream::pair().unwrap();
        read.set_read_timeout(Some(IO_TIMEOUT)).unwrap();
        let shared = Arc::new(Shared::default());
        *shared.socket.lock().unwrap() = Some(read.try_clone().unwrap());
        let worker_shared = shared.clone();
        let worker = thread::spawn(move || {
            let mut bytes = [0; 4];
            read_exact(&mut read, &mut bytes, &worker_shared).unwrap();
            assert_eq!(bytes, [1, 2, 3, 4]);
            read_exact(&mut read, &mut bytes, &worker_shared)
        });
        write.write_all(&[1, 2]).unwrap();
        thread::sleep(Duration::from_millis(5));
        write.write_all(&[3, 4]).unwrap();
        thread::sleep(Duration::from_millis(20));
        shared.stop();
        assert!(worker.join().unwrap().is_err());
    }
}

//! Loopback-only framing for the experiment, outside TinyStore's wire protocol.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::node::{Limits, Node};
use crate::packet::{MAX_PACKET, check_stream};
use crate::{Result, require};

const MAX_BATCH: usize = 32 * 1024 * 1024;

pub(super) struct Server {
    pub address: SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    counters: Arc<Counters>,
}

#[derive(Default)]
struct Counters {
    requests: AtomicU64,
    response_bytes: AtomicU64,
}

impl Server {
    pub(super) fn start(node: Arc<Mutex<Node>>) -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let address = listener.local_addr()?;
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = Arc::clone(&stop);
        let counters = Arc::new(Counters::default());
        let counted = Arc::clone(&counters);
        let thread = thread::spawn(move || {
            for socket in listener.incoming() {
                if stopping.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(mut socket) = socket else {
                    break;
                };
                // A broken client loses its response, never the durable feed.
                let _ = serve(&mut socket, &node, &counted);
            }
        });
        Ok(Self {
            address,
            stop,
            thread: Some(thread),
            counters,
        })
    }

    pub(super) fn counts(&self) -> (u64, u64) {
        (
            self.counters.requests.load(Ordering::SeqCst),
            self.counters.response_bytes.load(Ordering::SeqCst),
        )
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect_timeout(&self.address, Duration::from_secs(1));
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub(super) fn follow(node: &mut Node, upstream: SocketAddr, limits: Limits) -> Result<usize> {
    let mut socket = TcpStream::connect_timeout(&upstream, Duration::from_secs(2))?;
    socket.set_read_timeout(Some(Duration::from_secs(5)))?;
    socket.write_all(&[0])?;
    let count = read_u32(&mut socket)?;
    require(
        count <= 4096,
        "catalog exceeds the experiment's stream limit",
    )?;
    let mut names = Vec::new();
    for _ in 0..count {
        names.push(String::from_utf8(read_frame(&mut socket, 64)?)?);
    }
    let mut applied = 0;
    for stream in names {
        applied += sync(node, upstream, &stream, limits)?;
    }
    Ok(applied)
}

pub(super) fn sync(
    node: &mut Node,
    upstream: SocketAddr,
    stream: &str,
    limits: Limits,
) -> Result<usize> {
    check_stream(stream)?;
    require(
        limits.entries <= 1024 && limits.bytes <= MAX_BATCH,
        "transport batch limit exceeded",
    )?;
    let after = node
        .position(stream)?
        .map_or(0, |position| position.sequence);
    let mut socket = TcpStream::connect_timeout(&upstream, Duration::from_secs(2))?;
    socket.set_read_timeout(Some(Duration::from_secs(5)))?;
    socket.set_write_timeout(Some(Duration::from_secs(5)))?;
    socket.write_all(&[stream.len() as u8])?;
    socket.write_all(stream.as_bytes())?;
    socket.write_all(&after.to_be_bytes())?;
    socket.write_all(&(limits.entries as u32).to_be_bytes())?;
    socket.write_all(&(limits.bytes as u32).to_be_bytes())?;
    let count = read_u32(&mut socket)? as usize;
    if count == u32::MAX as usize {
        let bytes = read_frame(&mut socket, 1024)?;
        return Err(std::io::Error::other(String::from_utf8_lossy(&bytes).to_string()).into());
    }
    require(count <= limits.entries, "upstream exceeded entry limit")?;
    let mut total = 0;
    for _ in 0..count {
        let packet = read_frame(
            &mut socket,
            limits.bytes.saturating_sub(total).min(MAX_PACKET),
        )?;
        total += packet.len();
        let decoded = crate::packet::Packet::decode(&packet)?;
        require(
            decoded.stream == stream,
            "upstream returned the wrong stream",
        )?;
        node.receive(&packet)?;
    }
    Ok(count)
}

fn serve(socket: &mut TcpStream, node: &Mutex<Node>, counters: &Counters) -> Result<()> {
    socket.set_read_timeout(Some(Duration::from_secs(2)))?;
    socket.set_write_timeout(Some(Duration::from_secs(5)))?;
    let mut length = [0];
    socket.read_exact(&mut length)?;
    if length[0] == 0 {
        let streams = node
            .lock()
            .map_err(|_| std::io::Error::other("node lock poisoned"))?
            .streams()?;
        require(
            streams.len() <= 4096,
            "catalog exceeds the experiment's stream limit",
        )?;
        socket.write_all(&(streams.len() as u32).to_be_bytes())?;
        for stream in streams {
            write_frame(socket, stream.as_bytes())?;
        }
        return Ok(());
    }
    require(length[0] <= 64, "stream id exceeds the length limit")?;
    let mut name = vec![0; length[0] as usize];
    socket.read_exact(&mut name)?;
    let stream = String::from_utf8(name)?;
    let mut position = [0; 8];
    socket.read_exact(&mut position)?;
    let after = u64::from_be_bytes(position);
    let limits = Limits {
        entries: read_u32(socket)? as usize,
        bytes: read_u32(socket)? as usize,
    };
    require(
        limits.entries <= 1024 && limits.bytes <= MAX_BATCH,
        "transport batch limit exceeded",
    )?;
    // Copy bounded immutable packets, then release the node before network I/O.
    let response = node
        .lock()
        .map_err(|_| std::io::Error::other("node lock poisoned"))?
        .changes(&stream, after, limits);
    match response {
        Ok(packets) => {
            let bytes = 4 + packets.iter().map(|p| 4 + p.len() as u64).sum::<u64>();
            socket.write_all(&(packets.len() as u32).to_be_bytes())?;
            for packet in packets {
                write_frame(socket, &packet)?;
            }
            counters.requests.fetch_add(1, Ordering::SeqCst);
            counters.response_bytes.fetch_add(bytes, Ordering::SeqCst);
        }
        Err(error) => {
            socket.write_all(&u32::MAX.to_be_bytes())?;
            let message = error.to_string();
            write_frame(socket, &message.as_bytes()[..message.len().min(1024)])?;
        }
    }
    Ok(())
}

fn write_frame(socket: &mut TcpStream, bytes: &[u8]) -> Result<()> {
    socket.write_all(&(bytes.len() as u32).to_be_bytes())?;
    socket.write_all(bytes)?;
    Ok(())
}

fn read_u32(socket: &mut TcpStream) -> Result<u32> {
    let mut bytes = [0; 4];
    socket.read_exact(&mut bytes)?;
    Ok(u32::from_be_bytes(bytes))
}

fn read_frame(socket: &mut TcpStream, limit: usize) -> Result<Vec<u8>> {
    let length = read_u32(socket)? as usize;
    require(length <= limit, "frame exceeds the remaining byte budget")?;
    let mut bytes = vec![0; length];
    socket.read_exact(&mut bytes)?;
    Ok(bytes)
}

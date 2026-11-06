//! A socket that remembers when it last read a byte, so that a peer sending one long message is alive while its
//! bytes come, though no message is whole yet: a WebSocket library hands over a message only once it is complete.

use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::task::{Context, Poll};
use std::time::{Duration, Instant};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

/// When a socket last read a byte, as milliseconds since it was opened.
#[derive(Clone)]
pub struct Liveness {
    opened: Instant,
    last_read: Arc<AtomicU64>,
}

impl Liveness {
    pub fn new() -> Liveness {
        Liveness { opened: Instant::now(), last_read: Arc::new(AtomicU64::new(0)) }
    }

    fn mark(&self) {
        let now = u64::try_from(self.opened.elapsed().as_millis()).unwrap_or(u64::MAX);
        self.last_read.store(now, Ordering::Relaxed);
    }

    pub fn silent_for(&self) -> Duration {
        let last = Duration::from_millis(self.last_read.load(Ordering::Relaxed));
        self.opened.elapsed().saturating_sub(last)
    }
}

pub struct Watched<S> {
    inner: S,
    liveness: Liveness,
}

impl<S> Watched<S> {
    pub fn new(inner: S, liveness: Liveness) -> Watched<S> {
        Watched { inner, liveness }
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for Watched<S> {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        let before = buf.filled().len();
        let polled = Pin::new(&mut self.inner).poll_read(cx, buf);
        if buf.filled().len() > before {
            self.liveness.mark();
        }
        polled
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for Watched<S> {
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

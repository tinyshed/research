//! Research adapter: real Core Pipe for handshake/ordinary traffic; private
//! experimental channel methods at its boundary. No production router changes.
use std::{
    collections::{HashMap, VecDeque},
    ffi::{CStr, c_char},
    sync::{Arc, Mutex},
    time::Duration,
};
use tinystore::{Store, pipe::Pipe};
use tokio::sync::broadcast;

fn cpu_s() -> f64 {
    let mut r = std::mem::MaybeUninit::<libc::rusage>::uninit();
    unsafe {
        assert_eq!(libc::getrusage(libc::RUSAGE_SELF, r.as_mut_ptr()), 0);
        let r = r.assume_init();
        (r.ru_utime.tv_sec + r.ru_stime.tv_sec) as f64
            + (r.ru_utime.tv_usec + r.ru_stime.tv_usec) as f64 / 1e6
    }
}
const CAP: usize = 256;
type Payload = Arc<Vec<u8>>;
struct State {
    next: u64,
    ring: VecDeque<(u64, Payload)>,
    tx: broadcast::Sender<Payload>,
    published: u64,
    sent: u64,
    gaps: u64,
    active: u64,
    connections: u64,
}
pub struct Hub {
    store: Store,
    bucket: tinystore::kv::Bucket<String>,
    state: Mutex<State>,
    ring: bool,
}
struct Subscription {
    cursor: u64,
    receiver: Option<broadcast::Receiver<Payload>>,
    credit: u64,
}
pub struct Connection {
    hub: Arc<Hub>,
    pipe: Pipe,
    input: Vec<u8>,
    out: VecDeque<u8>,
    subs: HashMap<u32, Subscription>,
    turn: usize,
}
fn frame(out: &mut VecDeque<u8>, kind: u8, flags: u8, id: u32, body: &[u8]) {
    out.extend((body.len() as u32).to_le_bytes());
    out.extend([kind, flags, 0, 0]);
    out.extend(id.to_le_bytes());
    out.extend(body);
}
impl Hub {
    pub fn open(dir: &str, ring: bool) -> Arc<Self> {
        let store = Store::open(dir, Default::default()).unwrap();
        let bucket = store
            .bucket::<String>("channel-commit-marker")
            .open()
            .unwrap();
        let (tx, _) = broadcast::channel(CAP);
        Arc::new(Self {
            store,
            bucket,
            ring,
            state: Mutex::new(State {
                next: 0,
                ring: VecDeque::with_capacity(CAP),
                tx,
                published: 0,
                sent: 0,
                gaps: 0,
                active: 0,
                connections: 0,
            }),
        })
    }
    pub fn connect(self: &Arc<Self>) -> Connection {
        self.state.lock().unwrap().connections += 1;
        Connection {
            hub: self.clone(),
            pipe: Pipe::connect(&self.store, Default::default()).unwrap(),
            input: vec![],
            out: VecDeque::new(),
            subs: HashMap::new(),
            turn: 0,
        }
    }
}
impl Connection {
    pub fn push(&mut self, bytes: &[u8]) {
        self.input.extend_from_slice(bytes);
        while self.input.len() >= 12 {
            let n = u32::from_le_bytes(self.input[..4].try_into().unwrap()) as usize;
            assert!(n <= 1 << 20);
            if self.input.len() < 12 + n {
                break;
            }
            let f: Vec<u8> = self.input.drain(..12 + n).collect();
            let kind = f[4];
            let method = u16::from_le_bytes(f[6..8].try_into().unwrap());
            let id = u32::from_le_bytes(f[8..12].try_into().unwrap());
            let body = &f[12..];
            match (kind, method) {
                (3, 60000) => {
                    let mut state = self.hub.state.lock().unwrap();
                    let receiver = if self.hub.ring {
                        None
                    } else {
                        Some(state.tx.subscribe())
                    };
                    self.subs.insert(
                        id,
                        Subscription {
                            cursor: state.next,
                            receiver,
                            credit: if body.len() == 4 {
                                u32::from_le_bytes(body.try_into().unwrap()) as u64
                            } else {
                                2 << 20
                            },
                        },
                    );
                    state.active += 1;
                    frame(&mut self.out, 4, 0, id, &[]);
                    frame(&mut self.out, 7, 0, 0, &(n as u32).to_le_bytes());
                }
                (3, 60001) | (3, 60002) | (3, 60005) => {
                    // Serialize completion -> sequence assignment. This is a research
                    // adapter, not a measured grouped-commit notification hook.
                    let mut state = self.hub.state.lock().unwrap();
                    let seq = state.next;
                    if method == 60002 {
                        let r = self.hub.store.tx(|tx| -> tinystore::Result<()> {
                            tx.with(&self.hub.bucket).set("last", &seq.to_string())?;
                            Err(tinystore::Error::invalid("research rollback"))
                        });
                        assert!(r.is_err());
                    } else {
                        if method == 60001 {
                            self.hub.bucket.set("last", &seq.to_string()).unwrap();
                        }
                        let payload = Arc::new(body.to_vec());
                        if self.hub.ring {
                            if state.ring.len() == CAP {
                                state.ring.pop_front();
                            }
                            state.ring.push_back((seq, payload));
                        } else {
                            let _ = state.tx.send(payload);
                        }
                        state.next += 1;
                        state.published += 1;
                    }
                    frame(&mut self.out, 4, 1, id, &seq.to_le_bytes());
                    frame(&mut self.out, 7, 0, 0, &(n as u32).to_le_bytes());
                }
                (3, 60003) => {
                    let s = self.hub.state.lock().unwrap();
                    let status = serde_json::json!({"cpu_s":cpu_s(),"published":s.published,"wire_sent":s.sent,"gap_frames":s.gaps,"active_subscriptions":s.active,"connections":s.connections,"retained_payloads":if self.hub.ring{s.ring.len()}else{s.tx.len()},"capacity":CAP,"rss":std::fs::read_to_string("/proc/self/status").unwrap().lines().filter(|l|l.starts_with("VmRSS:")||l.starts_with("VmHWM:")).collect::<Vec<_>>()});
                    frame(&mut self.out, 4, 1, id, status.to_string().as_bytes());
                    frame(&mut self.out, 7, 0, 0, &(n as u32).to_le_bytes());
                }
                (7, 0) if self.subs.contains_key(&id) => {
                    self.subs.get_mut(&id).unwrap().credit +=
                        u32::from_le_bytes(body.try_into().unwrap()) as u64;
                }
                (6, 0) if self.subs.contains_key(&id) => {
                    self.subs.remove(&id);
                    self.hub.state.lock().unwrap().active -= 1;
                    frame(&mut self.out, 5, 1, id, &[]);
                }
                _ => self.out.extend(self.pipe.send(&f)),
            }
        }
    }
    pub fn take(&mut self, limit: usize) -> Vec<u8> {
        self.out.extend(self.pipe.recv(Duration::ZERO));
        let mut state = self.hub.state.lock().unwrap();
        let ids: Vec<u32> = self.subs.keys().copied().collect();
        let count = ids.len();
        for offset in 0..count {
            let index = (self.turn + offset) % count;
            let id = ids[index];
            let sub = self.subs.get_mut(&id).unwrap();
            for _ in 0..32 {
                if self.out.len() + 1040 > limit || sub.credit < 1024 {
                    break;
                }
                let event = if self.hub.ring {
                    let oldest = state.ring.front().map(|v| v.0).unwrap_or(state.next);
                    if sub.cursor < oldest {
                        let missed = oldest - sub.cursor;
                        sub.cursor = oldest;
                        Err(missed)
                    } else if sub.cursor < state.next {
                        let p = state.ring[(sub.cursor - oldest) as usize].1.clone();
                        sub.cursor += 1;
                        Ok(Some(p))
                    } else {
                        Ok(None)
                    }
                } else {
                    match sub.receiver.as_mut().unwrap().try_recv() {
                        Ok(p) => Ok(Some(p)),
                        Err(broadcast::error::TryRecvError::Lagged(n)) => Err(n),
                        Err(
                            broadcast::error::TryRecvError::Empty
                            | broadcast::error::TryRecvError::Closed,
                        ) => Ok(None),
                    }
                };
                match event {
                    Ok(Some(payload)) => {
                        assert!(payload.len() <= 1024);
                        sub.credit -= payload.len() as u64;
                        state.sent += 1;
                        frame(&mut self.out, 5, 0, id, &payload);
                    }
                    Ok(None) => break,
                    Err(missed) => {
                        state.gaps += 1;
                        let mut b = vec![255; 1024];
                        b[..8].copy_from_slice(&missed.to_le_bytes());
                        sub.credit -= 1024;
                        frame(&mut self.out, 5, 0, id, &b);
                    }
                }
            }
        }
        self.turn = if count == 0 {
            0
        } else {
            (self.turn + limit / 1024 / 32 + 1) % count
        };
        let n = limit.min(self.out.len());
        self.out.drain(..n).collect()
    }
}
impl Drop for Connection {
    fn drop(&mut self) {
        let mut s = self.hub.state.lock().unwrap();
        s.active -= self.subs.len() as u64;
        s.connections -= 1;
    }
}

// All FFI pointers originate in the matching create call and are used serially
// by the Bun event loop. Buffers remain borrowed for each call only.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lab_open(dir: *const c_char, ring: bool) -> *mut Arc<Hub> {
    let dir = unsafe { CStr::from_ptr(dir) }.to_str().unwrap();
    Box::into_raw(Box::new(Hub::open(dir, ring)))
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lab_connect(hub: *const Arc<Hub>) -> *mut Connection {
    Box::into_raw(Box::new(unsafe { &*hub }.connect()))
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lab_send(c: *mut Connection, bytes: *const u8, n: usize) {
    unsafe { &mut *c }.push(unsafe { std::slice::from_raw_parts(bytes, n) });
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lab_recv(c: *mut Connection, bytes: *mut u8, n: usize) -> usize {
    let out = unsafe { &mut *c }.take(n);
    unsafe { std::ptr::copy_nonoverlapping(out.as_ptr(), bytes, out.len()) };
    out.len()
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lab_disconnect(c: *mut Connection) {
    drop(unsafe { Box::from_raw(c) });
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lab_close(hub: *mut Arc<Hub>) {
    drop(unsafe { Box::from_raw(hub) });
}

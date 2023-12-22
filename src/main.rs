use std::collections::hash_map::DefaultHasher;
use std::collections::{HashSet, VecDeque};
use std::hash::{Hash, Hasher};
use std::io::{self, Read, Write};
use std::os::unix::net::{SocketAddr, UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::Mutex;
use std::time::{self, Duration, SystemTime};

use crate::socket_protocol::Message;

use self::socket_protocol::ToWriterError;

mod socket_protocol;

#[derive(Debug)]
pub struct Socket {
    stream: UnixStream,
}

impl Socket {
    pub fn new_path(idx: usize) -> std::io::Result<String> {
        let socket_path = format!("/tmp/vsm-{idx}");

        if Path::new(&socket_path).exists() {
            let _ = std::fs::remove_file(&socket_path);
        }

        Ok(socket_path)
    }

    pub fn new_server(socket_path: &str) -> std::io::Result<Socket> {
        let socket_listener = UnixListener::bind(&socket_path)?;
        let (stream, _socket_addr) = socket_listener.accept()?;

        let socket = Self { stream };

        Ok(socket)
    }

    pub fn new_fork(server_pair: &mut Self, idx: usize) -> std::io::Result<Socket> {
        let socket_path = Self::new_path(idx + 1)?;

        let socket_listener = UnixListener::bind(&socket_path)?;
        server_pair
            .send_message(&Message::Fork { socket_path })
            .unwrap();
        let (stream, _socket_addr) = socket_listener.accept()?;

        let socket = Self { stream };

        Ok(socket)
    }

    pub fn try_read_message(&mut self) -> Option<Message> {
        use socket_protocol::FromReader;

        self.stream.set_nonblocking(true).unwrap();
        let msg = Message::from_reader(&mut self.stream);
        self.stream.set_nonblocking(false).unwrap();

        msg.ok()
    }

    pub fn read_message(&mut self) -> Result<Message, ()> {
        use socket_protocol::FromReader;
        let msg = Message::from_reader(&mut self.stream).map_err(|_| ())?;
        Ok(msg)
    }

    pub fn send_message(&mut self, msg: &Message) -> Result<(), ToWriterError> {
        use socket_protocol::ToWriter;

        msg.to_writer(&mut self.stream)?;
        self.stream.flush()?;

        Ok(())
    }
}

pub struct ForkServer {
    server: Child,
    server_sockets: Socket,
    forks: Vec<Socket>,
}

impl ForkServer {
    pub fn new(cmd: &str) -> std::io::Result<Self> {
        static ALREADY_CREATED: AtomicBool = AtomicBool::new(false);

        let is_already_created =
            ALREADY_CREATED.fetch_or(true, std::sync::atomic::Ordering::SeqCst);

        if is_already_created {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Fork server already exists",
            ));
        }

        let socket_path = Socket::new_path(0)?;

        let mut server = Command::new(cmd);

        server.arg(&socket_path);

        server.stdin(Stdio::null());
        server.stdout(Stdio::inherit());
        server.stderr(Stdio::inherit());

        let server = server.spawn()?;
        let server_sockets = Socket::new_server(&socket_path)?;

        let fork_server = Self {
            server,
            server_sockets,

            forks: Vec::new(),
        };

        Ok(fork_server)
    }

    pub fn create_fork(&mut self) -> std::io::Result<usize> {
        let idx = self.forks.len();
        let socket = Socket::new_fork(&mut self.server_sockets, idx)?;
        self.forks.push(socket);

        Ok(idx)
    }

    pub fn replace_fork(&mut self, at: usize) -> std::io::Result<()> {
        let pair = Socket::new_fork(&mut self.server_sockets, at)?;

        self.forks[at] = pair;

        Ok(())
    }
}

pub struct FuzzInput {
    content: u64,
}

pub struct FuzzServer {
    fork_server: ForkServer,
    mutator: FuzzMutator,
    queue: VecDeque<FuzzInput>,
}

pub enum FuzzFinish {
    Data { hash: u64, data: Vec<u8> },
    InvalidResponse,
}

pub trait Monitor {
    fn init() -> Self;
    fn on_loop(&mut self, num_iters: u64);
}

pub struct NullMonitor;
pub struct IterationMonitor<const N: u64> {
    last_time: SystemTime,
    last_report: u64,
}

impl<const N: u64> Monitor for IterationMonitor<N> {
    fn init() -> Self {
        Self {
            last_time: time::SystemTime::now(),
            last_report: 0,
        }
    }

    fn on_loop(&mut self, num_iters: u64) {
        let delta_iters = num_iters - self.last_report;
        if delta_iters > N {
            let delta_time = self.last_time.elapsed().unwrap().as_secs_f64();
            let nanos_per_fuzz = (delta_time * 1000000.) / (delta_iters as f64);

            let (units_per_fuzz, unit) = if nanos_per_fuzz > 1_000_000. {
                (nanos_per_fuzz * 1_000_000., "s")
            } else if nanos_per_fuzz > 1_000. {
                (nanos_per_fuzz * 1_000., "ms")
            } else {
                (nanos_per_fuzz, "ns")
            };

            println!("[MONITOR]: {num_iters} iterations done ({units_per_fuzz:.03}{unit} / fuzz over last {delta_iters})");

            self.last_report = num_iters;
            self.last_time = SystemTime::now();
        }
    }
}

impl Monitor for NullMonitor {
    #[inline]
    fn init() -> Self {
        Self
    }
    #[inline]
    fn on_loop(&mut self, _num_iters: u64) {}
}

pub struct FuzzMutator {
    idx: u32,
}

impl FuzzMutator {
    pub fn new() -> Self {
        Self { idx: 0 }
    }
}

impl FuzzServer {
    pub fn new(cmd: &str) -> io::Result<Self> {
        let fork_server = ForkServer::new(&cmd)?;

        Ok(Self {
            fork_server,
            mutator: FuzzMutator::new(),
            queue: VecDeque::new(),
        })
    }

    fn on_exec(&mut self, idx: usize) -> Result<(), ToWriterError> {
        let data = self.mutator.idx.to_le_bytes().to_vec();
        self.mutator.idx += 1;

        self.fork_server.forks[idx].send_message(&Message::Data { content: data })?;

        Ok(())
    }

    fn try_finish(fork: &mut Socket) -> Option<FuzzFinish> {
        fork.try_read_message().map(|msg| match msg {
            Message::Data { content } => {
                let mut hasher = DefaultHasher::new();
                content.hash(&mut hasher);
                let hash = hasher.finish();

                FuzzFinish::Data {
                    hash,
                    data: content,
                }
            }
            _ => {
                eprintln!("[WARN]: Did not receive a data message");
                FuzzFinish::InvalidResponse
            }
        })
    }

    pub fn fuzz_loop<M: Monitor>(
        &mut self,
        num_children: u16,
        iterations: Option<u64>,
    ) -> Result<(), ToWriterError> {
        let mut num_iters = 0;

        for _ in 0..num_children {
            num_iters += 1;

            let idx = self.fork_server.create_fork()?;
            self.on_exec(idx)?;
        }

        let mut fork_offset = 0;

        let mut seen_covmaps = HashSet::new();

        let mut monitor = M::init();
        loop {
            fork_offset += 1;
            fork_offset %= num_children;

            monitor.on_loop(num_iters);

            let fork = &mut self.fork_server.forks[fork_offset as usize];
            if let Some(fuzz_finish) = Self::try_finish(fork) {
                match fuzz_finish {
                    FuzzFinish::Data { hash, data: _ } => {
                        let is_new = seen_covmaps.insert(hash);
                        //
                        // if is_new {
                        //     println!("Is new");
                        // }
                    }
                    _ => {}
                };

                num_iters += 1;
                if iterations.is_some_and(|iterations| iterations <= num_iters) {
                    break;
                }

                self.fork_server.replace_fork(fork_offset as usize).unwrap();
                self.on_exec(fork_offset as usize)?;
            }
        }

        println!("[FUZZ SERVER]: Done with iterations");

        Ok(())
    }

    pub fn clean(mut self) -> io::Result<()> {
        self.fork_server
            .server_sockets
            .send_message(&Message::Exit)
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "Failed to send exit message"))?;
        self.fork_server.server.wait()?;

        Ok(())
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cmd = format!(
        "/home/johndoe/Downloads/software_verilator/software/obj_dir/VSECURE_PLATFORM_RI5CY_CW",
    );

    let mut fuzz_server = FuzzServer::new(&cmd)?;
    fuzz_server.fuzz_loop::<IterationMonitor<1000>>(20, Some(10_000))?;
    fuzz_server.clean()?;

    Ok(())
}

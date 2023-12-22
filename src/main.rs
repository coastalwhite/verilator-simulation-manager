use std::collections::hash_map::DefaultHasher;
use std::collections::{HashSet, VecDeque};
use std::hash::{Hash, Hasher};
use std::io::{self, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::time::{self, Duration, SystemTime};

use crate::socket_protocol::{Message, ProtocolError};

use self::socket_protocol::ProtocolResult;

mod socket_protocol;

#[derive(Debug)]
pub struct Socket {
    stream: UnixStream,
}

impl Socket {
    pub fn new_path(idx: usize) -> ProtocolResult<String> {
        let socket_path = format!("/tmp/vsm-{idx}");

        if Path::new(&socket_path).exists() {
            let _ = std::fs::remove_file(&socket_path);
        }

        Ok(socket_path)
    }

    pub fn new_server(socket_path: &str) -> ProtocolResult<Socket> {
        let socket_listener = UnixListener::bind(&socket_path)?;
        let (stream, _socket_addr) = socket_listener.accept()?;

        stream.set_read_timeout(Some(Duration::from_secs(1)))?;
        stream.set_write_timeout(Some(Duration::from_secs(1)))?;

        let socket = Self { stream };

        Ok(socket)
    }

    pub fn new_fork(server_pair: &mut Self, idx: usize) -> ProtocolResult<Socket> {
        let socket_path = Self::new_path(idx + 1)?;

        let socket_listener = UnixListener::bind(&socket_path)?;
        server_pair
            .send_message(&Message::Fork { socket_path })
            .unwrap();
        let (stream, _socket_addr) = socket_listener.accept()?;

        stream.set_nonblocking(true)?;
        let socket = Self { stream };

        Ok(socket)
    }

    pub fn try_read_message(&mut self) -> Option<Message> {
        use socket_protocol::FromReader;
        Message::from_reader(&mut self.stream)
            .map_err(|err| {
                if !matches!(&err, ProtocolError::Io(io_err) if io_err.kind() == std::io::ErrorKind::WouldBlock) {
                    dbg!(&err);
                }
                err
            })
            .ok()
    }

    pub fn read_message(&mut self) -> ProtocolResult<Message> {
        use socket_protocol::FromReader;
        let msg = Message::from_reader(&mut self.stream)?;
        Ok(msg)
    }

    pub fn send_message(&mut self, msg: &Message) -> ProtocolResult<()> {
        use socket_protocol::ToWriter;

        msg.to_writer(&mut self.stream)?;
        self.stream.flush()?;

        Ok(())
    }
}

pub struct ForkServer {
    server: Child,
    server_socket: Socket,
    forks: Vec<Socket>,
}

impl ForkServer {
    pub fn new(cmd: &str) -> ProtocolResult<Self> {
        static ALREADY_CREATED: AtomicBool = AtomicBool::new(false);

        let is_already_created =
            ALREADY_CREATED.fetch_or(true, std::sync::atomic::Ordering::SeqCst);

        if is_already_created {
            return Err(ProtocolError::other("Fork server already exists"));
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
            server_socket: server_sockets,

            forks: Vec::new(),
        };

        Ok(fork_server)
    }

    pub fn create_fork(&mut self) -> ProtocolResult<usize> {
        let idx = self.forks.len();
        let socket = Socket::new_fork(&mut self.server_socket, idx)?;
        self.forks.push(socket);

        Ok(idx)
    }

    pub fn replace_fork(&mut self, at: usize) -> ProtocolResult<()> {
        let pair = Socket::new_fork(&mut self.server_socket, at)?;

        self.forks[at] = pair;

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FuzzInput {
    content: Vec<u8>,
}

pub struct FuzzServer {
    fork_server: ForkServer,

    current_datas: Vec<Option<FuzzInput>>,

    mutator: FuzzMutator,

    run_queue: Vec<FuzzInput>,
    mutation_queue: Vec<FuzzInput>,
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
            let nanos_per_fuzz = (delta_time * 1_000_000.) / (delta_iters as f64);

            let (units_per_fuzz, unit) = if nanos_per_fuzz > 1_000_000. {
                (nanos_per_fuzz / 1_000_000., "s")
            } else if nanos_per_fuzz > 1_000. {
                (nanos_per_fuzz / 1_000., "ms")
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
    width: u32,
    seen: HashSet<FuzzInput>,
}

impl FuzzMutator {
    pub fn new(width: u32) -> Self {
        Self {
            width,
            seen: HashSet::new(),
        }
    }

    pub fn deterministic_mutate(&mut self, queue: &mut Vec<FuzzInput>, input: &[u8]) {
        assert!(self.width.is_power_of_two());
        for i in 0..self.width {
            let byte_off = i / 8;
            let bit_off = i % 8;

            let mut new_fuzz = input.to_vec();
            new_fuzz[byte_off as usize] ^= 1 << bit_off;
            let new_fuzz = FuzzInput { content: new_fuzz };

            if self.seen.insert(new_fuzz.clone()) {
                queue.push(new_fuzz);
            }
        }

        for i in 0..self.width / 4 {
            let byte_off = i / 2;
            let bit_off = i % 8;

            let mut new_fuzz = input.to_vec();
            new_fuzz[byte_off as usize] ^= 0xF << bit_off;
            let new_fuzz = FuzzInput { content: new_fuzz };

            if self.seen.insert(new_fuzz.clone()) {
                queue.push(new_fuzz);
            }
        }

        for i in 0..self.width / 8 {
            let mut new_fuzz = input.to_vec();
            new_fuzz[i as usize] ^= 0xFF;
            let new_fuzz = FuzzInput { content: new_fuzz };

            if self.seen.insert(new_fuzz.clone()) {
                queue.push(new_fuzz);
            }
        }

        for i in 0..self.width / 8 {
            for j in 0..35 {
                let mut new_fuzz = input.to_vec();
                new_fuzz[i as usize] = new_fuzz[i as usize].wrapping_add(j);
                let new_fuzz = FuzzInput { content: new_fuzz };

                if self.seen.insert(new_fuzz.clone()) {
                    queue.push(new_fuzz);
                }

                let mut new_fuzz = input.to_vec();
                new_fuzz[i as usize] = new_fuzz[i as usize].wrapping_sub(j);
                let new_fuzz = FuzzInput { content: new_fuzz };

                if self.seen.insert(new_fuzz.clone()) {
                    queue.push(new_fuzz);
                }
            }
        }
    }
}

impl FuzzServer {
    pub fn new(cmd: &str) -> ProtocolResult<Self> {
        let mut fork_server = ForkServer::new(&cmd)?;

        let Message::InputWidth { width } = fork_server.server_socket.read_message()? else {
            return Err(ProtocolError::other(
                "Expected input width message from fork server",
            ));
        };

        Ok(Self {
            fork_server,
            current_datas: Vec::new(),
            mutator: FuzzMutator::new(width),
            run_queue: Vec::new(),
            mutation_queue: Vec::new(),
        })
    }

    fn on_exec(&mut self, idx: usize) -> ProtocolResult<()> {
        if self.run_queue.is_empty() {
            self.refresh_queue();
        }

        // dbg!(self.mutation_queue.len());
        // dbg!(self.run_queue.len());
        //
        // dbg!(&self.run_queue);
        // dbg!(&self.mutation_queue);
        //
        let data = self.run_queue.pop().unwrap();
        self.current_datas[idx] = Some(data.clone());
        self.fork_server.forks[idx].send_message(&Message::Data {
            content: data.content,
        })?;

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

    pub fn refresh_queue(&mut self) {
        println!("Refreshing Queue");

        for candidate in self.mutation_queue.iter() {
            self.mutator
                .deterministic_mutate(&mut self.run_queue, &candidate.content);
        }
        self.mutation_queue.clear();

        println!("New queue has {} inputs.", self.run_queue.len());
    }

    pub fn fuzz_loop<M: Monitor>(
        &mut self,
        num_children: u16,
        iterations: Option<u64>,
    ) -> ProtocolResult<()> {
        let mut num_iters = 0;

        for _ in 0..num_children {
            self.current_datas.push(None);

            num_iters += 1;

            let idx = self.fork_server.create_fork()?;
            self.on_exec(idx)?;
        }

        let mut fork_offset = 0;
        let mut branches_found = 0u64;

        let mut seen_covmaps = HashSet::new();

        let mut monitor = M::init();
        loop {
            fork_offset += 1;
            fork_offset %= num_children;

            monitor.on_loop(num_iters);

            let fork = &mut self.fork_server.forks[fork_offset as usize];
            if let Some(fuzz_finish) = Self::try_finish(fork) {
                fork.send_message(&Message::Exit)?;

                match fuzz_finish {
                    FuzzFinish::Data { hash, data: _ } => {
                        let is_new = seen_covmaps.insert(hash);

                        if is_new {
                            branches_found += 1;

                            if branches_found % 100 == 0 {
                                println!("Found {branches_found} branches");
                            }

                            self.mutation_queue.push(
                                self.current_datas[fork_offset as usize]
                                    .as_ref()
                                    .unwrap()
                                    .clone(),
                            );
                        }
                    }
                    _ => {}
                };

                num_iters += 1;
                if iterations.is_some_and(|iterations| iterations <= num_iters) {
                    break;
                }

                if self.run_queue.is_empty() {
                    self.refresh_queue();
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
            .server_socket
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
    let initial_width = fuzz_server.mutator.width;
    fuzz_server.mutation_queue.push(FuzzInput {
        content: vec![0; initial_width.div_ceil(8) as usize],
    });
    fuzz_server.fuzz_loop::<IterationMonitor<1000>>(20, Some(1_000_000))?;
    fuzz_server.clean()?;

    Ok(())
}

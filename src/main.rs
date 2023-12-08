use std::io::{Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::Mutex;
use std::time::Duration;

use crate::socket_protocol::Message;

mod socket_protocol;

#[derive(Debug)]
pub struct SocketPair {
    sc_socket: UnixStream,
    cs_socket: UnixStream,
}

impl SocketPair {
    pub fn new_paths() -> std::io::Result<(String, String)> {
        static IDX_CTR: AtomicU64 = AtomicU64::new(0);

        let idx = IDX_CTR.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

        let sc_socket_path = format!("/tmp/vsm-{idx}-sc");
        let cs_socket_path = format!("/tmp/vsm-{idx}-cs");

        if Path::new(&sc_socket_path).exists() {
            let _ = std::fs::remove_file(&sc_socket_path);
        }

        if Path::new(&cs_socket_path).exists() {
            let _ = std::fs::remove_file(&cs_socket_path);
        }

        Ok((sc_socket_path, cs_socket_path))
    }

    pub fn new_server(sc_socket_path: &str, cs_socket_path: &str) -> std::io::Result<SocketPair> {
        let cs_socket_listener = UnixListener::bind(&cs_socket_path)?;
        let (cs_socket, _cs_socket_addr) = cs_socket_listener.accept()?;

        let sc_socket = UnixStream::connect(&sc_socket_path)?;

        let pair = Self {
            sc_socket,
            cs_socket,
        };

        Ok(pair)
    }

    pub fn new_fork(server_pair: &mut Self) -> std::io::Result<SocketPair> {
        let (sc_socket_path, cs_socket_path) = Self::new_paths()?;

        let cs_socket_listener = UnixListener::bind(&cs_socket_path)?;

        server_pair
            .send_message(Message::Fork {
                sc_socket: sc_socket_path.to_string(),
                cs_socket: cs_socket_path.to_string(),
            })
            .unwrap();

        let (cs_socket, _cs_socket_addr) = cs_socket_listener.accept()?;
        let sc_socket = UnixStream::connect(&sc_socket_path)?;

        let pair = Self {
            sc_socket,
            cs_socket,
        };

        Ok(pair)
    }

    pub fn read_message(&mut self) -> Result<Message, ()> {
        use socket_protocol::FromReader;
        let msg = Message::from_reader(&mut self.cs_socket).map_err(|_| ())?;
        Ok(msg)
    }

    pub fn set_read_timeout(&mut self, timeout: Option<Duration>) -> std::io::Result<()> {
        self.cs_socket.set_read_timeout(timeout)?;
        Ok(())
    }

    pub fn send_message(&mut self, msg: Message) -> Result<(), ()> {
        use socket_protocol::ToWriter;
        msg.to_writer(&mut self.sc_socket).map_err(|_| ())?;
        self.sc_socket.flush().map_err(|_| ())?;
        Ok(())
    }
}

pub struct ForkServer {
    server: Child,
    server_sockets: SocketPair,

    forks: Vec<SocketPair>,
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

        let (sc_socket_path, cs_socket_path) = SocketPair::new_paths()?;

        let mut server = Command::new(cmd);

        server.arg(&sc_socket_path);
        server.arg(&cs_socket_path);

        server.stdin(Stdio::null());
        server.stdout(Stdio::inherit());
        server.stderr(Stdio::inherit());

        let server = server.spawn()?;
        let server_sockets = SocketPair::new_server(&sc_socket_path, &cs_socket_path)?;

        let fork_server = Self {
            server,
            server_sockets,

            forks: Vec::new(),
        };

        Ok(fork_server)
    }

    pub fn create_fork(&mut self) -> std::io::Result<usize> {
        let pair = SocketPair::new_fork(&mut self.server_sockets)?;

        let idx = self.forks.len();
        self.forks.push(pair);

        Ok(idx)
    }
}

fn main() {
    let cmd = format!(
        "{}/socket-test.py",
        std::env::current_dir().unwrap().to_str().unwrap()
    );
    let mut fork_server = ForkServer::new(&cmd).unwrap();

    for _ in 0..10 {
        let idx = fork_server.create_fork().unwrap();
        fork_server.forks[idx]
            .send_message(Message::Data {
                content: vec![0x13, 0x37, 0x42],
            })
            .unwrap();
    }

    loop {}
}

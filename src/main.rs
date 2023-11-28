use std::io::{Write, Read};
use std::os::unix::net::{UnixStream, UnixListener};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

use timeout_readwrite::TimeoutReader;

#[repr(u8)]
enum ForkerCommand {
    Ack = 42,
    Fork = 1,
}

fn wait_ack_response(forker_mgr_socket: &mut UnixStream) {
    let mut buf = [0u8; 1];
    forker_mgr_socket.read_exact(&mut buf).expect("Expected message from Forker");

    if buf[0] != ForkerCommand::Ack as u8 {
        eprintln!("[MANAGER]: ERROR. Expected an ACK from Forker");
        std::process::exit(1);
    }
}

fn ask_forker_fork(mgr_forker_sock: &mut UnixStream, forker_mgr_sock: &mut UnixStream) -> (UnixStream, UnixStream) {
    mgr_forker_sock.write_all(&[ForkerCommand::Fork as u8]).unwrap();
    mgr_forker_sock.flush().unwrap();

    wait_ack_response(forker_mgr_sock);

    static IDX_CTR: Mutex<u64> = Mutex::new(0);

    let mut guard = IDX_CTR.lock().unwrap();
    let idx = *guard;
    *guard = guard.checked_add(1).expect("Client Index overflowed");

    let client_mgr_socket_path = format!("/tmp/vsm-{idx}-client-mgr");
    let mgr_client_socket_path = format!("/tmp/vsm-{idx}-mgr-client");

    if Path::new(&client_mgr_socket_path).exists() {
        let _ = std::fs::remove_file(&client_mgr_socket_path).unwrap();
    }

    let client_mgr_sock_listener = UnixListener::bind(&client_mgr_socket_path).expect("Failed to connect to Client->Manager socket");

    mgr_forker_sock.write_all(&(mgr_client_socket_path.len() as u32).to_be_bytes()).unwrap();
    mgr_forker_sock.write_all(&(client_mgr_socket_path.len() as u32).to_be_bytes()).unwrap();

    write!(mgr_forker_sock, "{mgr_client_socket_path}").unwrap();
    write!(mgr_forker_sock, "{client_mgr_socket_path}").unwrap();
    mgr_forker_sock.flush().unwrap();

    println!("[MANAGER]: Waiting for acknowledge of socket paths");
    wait_ack_response(forker_mgr_sock);

    let mgr_client_sock = UnixStream::connect(mgr_client_socket_path).expect("Failed to connect to Manager->Client socket");
    let (client_mgr_sock, _client_mgr_sock_addr) = client_mgr_sock_listener.accept().expect("Failed to accept a connection for Client->Manager socket");

    println!("[MANAGER]: Established socket connection with client {idx}!");

    mgr_client_sock.set_write_timeout(Some(Duration::from_secs(3))).unwrap();
    client_mgr_sock.set_read_timeout(Some(Duration::from_secs(3))).unwrap();

    (mgr_client_sock, client_mgr_sock)
}

fn main() {
    let mut forker = Command::new("./socket-test.py");
        
    forker.stdin(Stdio::piped());
    forker.stdout(Stdio::piped());

    let forker_proc = forker.spawn().expect("Failed to start forker");

    let mut forker_stdin = forker_proc.stdin.expect("Failed to grab forker stdin");
    let forker_stdout = forker_proc.stdout.expect("Failed to grab forker stdout");

    let mut forker_stdout = TimeoutReader::new(forker_stdout, Duration::from_secs(3));

    writeln!(forker_stdin, "Hello Forker!").unwrap();
    forker_stdin.flush().unwrap();

    println!("[MANAGER]: Waiting for Hello");
    const HELLO_RESPONSE: &[u8] = b"Hello Manager!\n";
    let mut return_value = [0u8; HELLO_RESPONSE.len()];
    match forker_stdout.read_exact(&mut return_value) {
        Ok(_) if return_value == HELLO_RESPONSE => {},
        Ok(_) => panic!("Invalid hello response from forker: {}", String::from_utf8_lossy(&return_value)),
        Err(err) if err.kind() == std::io::ErrorKind::TimedOut => panic!("Forker hello response timed out"),
        Err(err) => panic!("Forker hello response error: {err:?}"),
    };

    const MGR_FORKER_SOCKET_PATH: &str = "/tmp/vsm-forker-mgr-forker";
    const FORKER_MGR_SOCKET_PATH: &str = "/tmp/vsm-forker-forker-mgr";

    if Path::new(FORKER_MGR_SOCKET_PATH).exists() {
        let _ = std::fs::remove_file(FORKER_MGR_SOCKET_PATH).unwrap();
    }

    let forker_mgr_sock_listener = UnixListener::bind(FORKER_MGR_SOCKET_PATH).expect("Failed to connect to Forker->Manager socket");

    writeln!(forker_stdin, "{MGR_FORKER_SOCKET_PATH}").unwrap();
    writeln!(forker_stdin, "{FORKER_MGR_SOCKET_PATH}").unwrap();
    forker_stdin.flush().unwrap();

    println!("[MANAGER]: Waiting for Sockets");
    const SOCKET_RESPONSE: &[u8] = b"Received Sockets!\n";
    let mut return_value = [0u8; SOCKET_RESPONSE.len()];
    match forker_stdout.read_exact(&mut return_value) {
        Ok(_) if return_value == SOCKET_RESPONSE => {},
        Ok(_) => panic!("Invalid socket response from forker: {}", String::from_utf8_lossy(&return_value)),
        Err(err) if err.kind() == std::io::ErrorKind::TimedOut => panic!("Forker socket response timed out"),
        Err(err) => panic!("Forker socket response error: {err:?}"),
    };

    let mut mgr_forker_sock = UnixStream::connect(MGR_FORKER_SOCKET_PATH).expect("Failed to connect to Manager->Forker socket");
    let (mut forker_mgr_sock, _forker_mgr_sock_addr) = forker_mgr_sock_listener.accept().expect("Failed to accept a connection for Forker->Manager socket");

    println!("[MANAGER]: Established socket connection!");

    drop(forker_stdin);
    drop(forker_stdout);

    mgr_forker_sock.set_write_timeout(Some(Duration::from_secs(3))).unwrap();
    forker_mgr_sock.set_read_timeout(Some(Duration::from_secs(3))).unwrap();

    let mut sockets = Vec::new();
    for i in 0..10 {
        sockets.push(ask_forker_fork(&mut mgr_forker_sock, &mut forker_mgr_sock));
        println!("[MANAGER]: Fork {i} done");
    }

    loop {}
}

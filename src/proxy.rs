use libtxc::{LibTxc, LogLevel};
use std::io::Read;
use std::os::windows::io::{FromRawSocket, IntoRawSocket, RawSocket};
use std::process::{Command, Stdio};
use std::{
    env,
    io::{self, BufRead, BufReader, Write},
    mem,
    net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream},
};

use winapi::um::winsock2::{
    closesocket, WSADuplicateSocketW, WSAGetLastError, WSASocketW, FROM_PROTOCOL_INFO,
    INVALID_SOCKET, SOCKET, WSAPROTOCOL_INFOW, WSA_FLAG_OVERLAPPED,
};

const TXC_PROXY_FORK_ENV: &str = "__TXC_PROXY_FORK";
const TXC_PROXY_LOG_LEVEL: &str = "TXC_PROXY_LOG_LEVEL";

#[inline(always)]
fn last_os_error() -> io::Error {
    io::Error::last_os_error()
}

#[inline(always)]
fn bind(port: u16) -> std::io::Result<TcpListener> {
    TcpListener::bind(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), port))
}

fn bind_any() -> Option<(u16, TcpListener)> {
    for port in 1025..65535 {
        if let Ok(listener) = bind(port) {
            return Some((port, listener));
        }
    }
    None
}

#[inline(always)]
fn load_lib() -> io::Result<LibTxc> {
    LibTxc::new(std::env::current_dir()?)
}

fn init_lib(mut lib: LibTxc, id: u16, mut data_stream: TcpStream) -> io::Result<LibTxc> {
    let log_level: LogLevel = match std::env::var(TXC_PROXY_LOG_LEVEL) {
        Ok(s) => s.parse::<u8>().unwrap_or(1).into(),
        _ => LogLevel::Minimum,
    };

    let wd = std::env::current_dir()?;
    let log_dir = wd.join("sessions").join(id.to_string());
    std::fs::create_dir_all(log_dir.clone())?;
    lib.initialize(log_dir, log_level)?;
    lib.set_callback(move |buff| data_stream.write_all(&*buff));
    Ok(lib)
}

fn handle_conn(mut cmd_stream: TcpStream) -> io::Result<()> {
    let lib = bind_any()
        .ok_or_else(last_os_error)
        .and_then(|(data_port, listener)| {
            // load here to fail early, in case
            let lib = load_lib()?;
            // send data port, wait for connection
            let (ds, _) = cmd_stream
                .write_all(&data_port.to_le_bytes())
                .and_then(|_| listener.accept())?;
            ds.shutdown(std::net::Shutdown::Read)?;
            init_lib(lib, data_port, ds)
        })?;

    let mut reader = BufReader::new(cmd_stream.try_clone()?);
    let mut buff = Vec::with_capacity(1 << 20);

    while !matches!(reader.read_until(b'\0', &mut buff), Ok(0) | Err(_)) {
        let resp = match lib.send_bytes(&buff) {
            Ok(resp) => resp,
            Err(e) => e.message,
        };
        cmd_stream.write_all(resp.as_bytes())?;
        buff.clear();
    }
    Ok(())
}

fn handler() -> io::Result<()> {
    // before using any winsock2 stuff it should be initialized(WSAStartup), let libstd handle this
    drop(std::net::TcpListener::bind("255.255.255.255:0"));

    env::remove_var(TXC_PROXY_FORK_ENV);
    // read socket info from stdin
    let mut buff = Vec::with_capacity(mem::size_of::<WSAPROTOCOL_INFOW>());
    std::io::stdin().read_to_end(&mut buff)?;
    // reconstruct socket
    let stream: TcpStream = unsafe {
        let pi: &mut WSAPROTOCOL_INFOW = &mut *(buff.as_ptr() as *mut WSAPROTOCOL_INFOW);
        let sock = WSASocketW(
            FROM_PROTOCOL_INFO,
            FROM_PROTOCOL_INFO,
            FROM_PROTOCOL_INFO,
            pi,
            0,
            WSA_FLAG_OVERLAPPED,
        );
        if sock == INVALID_SOCKET {
            return Err(io::Error::from_raw_os_error(WSAGetLastError()));
        }
        TcpStream::from_raw_socket(sock as RawSocket)
    };
    handle_conn(stream)
}

fn spawn_handler(stream: TcpStream) -> io::Result<()> {
    // fork
    let cmd = env::current_exe()?;
    let mut child = Command::new(cmd)
        .env(TXC_PROXY_FORK_ENV, "")
        .current_dir(env::current_dir()?)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .spawn()?;
    let pid = child.id();
    let sin = child.stdin.as_mut().ok_or_else(last_os_error)?;

    // duplicate socket
    let raw_fd = stream.into_raw_socket();
    let pl = unsafe {
        let mut pi: WSAPROTOCOL_INFOW = mem::zeroed();
        let rv = WSADuplicateSocketW(raw_fd as SOCKET, pid, &mut pi);
        if rv != 0 {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("socket dup fail {}", rv),
            ));
        }
        std::slice::from_raw_parts(
            mem::transmute::<_, *const u8>(&pi),
            mem::size_of::<WSAPROTOCOL_INFOW>(),
        )
    };
    // send socket info to child's stdin
    sin.write_all(pl)?;
    // finally close our copy of the socket
    unsafe { closesocket(raw_fd as SOCKET) };
    Ok(())
}

fn server() -> io::Result<()> {
    let mut control_port = 5555;
    for arg in env::args().rev() {
        if let Ok(p) = arg.parse::<u16>() {
            control_port = p;
            break;
        }
    }

    let (control_port, listener) = match bind(control_port) {
        Ok(l) => Ok((control_port, l)),
        Err(e) => {
            eprintln!("127.0.0.1:{} bind error {}", control_port, e);
            bind_any().ok_or_else(last_os_error)
        }
    }?;

    println!("Сервер запущен на: {}", control_port);
    for conn in listener.incoming() {
        conn.and_then(spawn_handler)?;
    }
    Ok(())
}

pub fn main() -> io::Result<()> {
    if env::var(TXC_PROXY_FORK_ENV).is_ok() {
        handler()
    } else {
        server()
    }
}

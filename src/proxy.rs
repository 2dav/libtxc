use libtxc::{LibTxc, LogLevel};
use std::io::{self, BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};

fn bind(port: u16) -> std::io::Result<TcpListener> {
    TcpListener::bind(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), port))
}

fn bind_random() -> Option<(u16, TcpListener)> {
    for port in 1025..65535 {
        if let Ok(listener) = bind(port) {
            return Some((listener.local_addr().unwrap().port(), listener));
        }
    }
    None
}

fn init_lib(port: u16, mut data_stream: TcpStream) -> io::Result<LibTxc> {
    let cd = std::env::current_dir()?;
    let mut lib = LibTxc::new(cd.clone())?;
    println!("{}: библиотека загружена", port);

    let cd = cd.join("sessions").join(format!("{}", port));
    std::fs::create_dir_all(cd.clone())?;
    lib.initialize(cd.clone(), LogLevel::Minimum)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
    println!("{}: логи коннектора сохраняются в {:?}", port, cd);

    lib.set_callback(move |buff| data_stream.write_all(&*buff));
    Ok(lib)
}

fn handle_conn(mut cmd_stream: TcpStream) {
    match bind_random()
        .ok_or(io::Error::last_os_error())
        .and_then(|(port, listener)| {
            println!("{}: порт данных открыт, ожидаю подключение", port);
            cmd_stream
                .write_all(&port.to_le_bytes())
                .and_then(|_| listener.accept())
                .and_then(|(data_stream, _)| init_lib(port, data_stream))
                .map(|lib| (lib, port))
        }) {
        Ok((lib, id)) => {
            println!("{}: инициализаця завершена, начинаю приём данных", id);
            let mut reader = BufReader::new(cmd_stream.try_clone().unwrap());
            let mut buff = Vec::with_capacity(1 << 20);

            while match reader.read_until(b'\0', &mut buff) {
                Ok(0) | Err(_) => false,
                _ => true,
            } {
                let resp = match lib.send_bytes(&buff) {
                    Ok(resp) => resp,
                    Err(e) => e.message,
                };
                if cmd_stream.write_all(resp.as_bytes()).is_err() {
                    break;
                }
                buff.clear();
            }
            println!("{}: завершаю работу, корректно", id);
        }
        Err(e) => eprintln!("{}", e),
    };
}

pub fn main() -> std::io::Result<()> {
    let mut control_port = 5555;
    for arg in std::env::args().rev() {
        if let Ok(p) = arg.parse::<u16>() {
            control_port = p;
            break;
        }
    }

    let listener = bind(control_port)?;
    println!("Сервер запущен на: {}", control_port);
    for conn in listener.incoming() {
        let stream = conn?;
        std::thread::spawn(move || handle_conn(stream));
    }
    Ok(())
}

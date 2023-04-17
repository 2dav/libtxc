include!("common/common.rs");

use libtxc::{LogLevel, Stream, TCStr, TransaqConnector};
use tracing::info;

// запуск примера:
// cargo run --release --example threading
//
// Пример многопоточного приложения
//
// Aрхитектура и используемые средства не являются оптимальными, но
// демонстрируют особенности использования в многопоточном окружении.
fn main() -> anyhow::Result<()> {
    let (login, password, lib, logdir) = init()?;
    init_logging();

    let mut txc = TransaqConnector::new(lib.into(), logdir.into(), LogLevel::Minimum)?;

    /* В этом приложении поступающие сообщения обрабатываются в 4х потоках:
    - поток 0 фильтрует и парсит приходящие сообщения, *управляется коннектором*
    - поток 1 отправляет команду "disconnect" при получении сообщения "connected = true"
    - поток 2 отправляет команду "connect" при получении сообщения "connected = false"
    - поток 3 регулярно отправляет не требующую активного соединения команду

       Каналы коммуникации

                ┌─[1] > TX
       RX > [0]─┼─[2] > TX
                └─[3] > TX

    Объект `Sender`, возвращаемый `TransaqConnector::sender()`, может быть клонирован и
    передан между потоками для создания нескольких точек отправки сообщений.*/

    let s1 = txc.sender();
    let s2 = s1.clone();
    let s3 = s1.clone();

    // Структура внутренних сообщений
    #[derive(Clone, Copy)]
    enum Message {
        Status(bool),
        Version(u8),
    }

    let cmd_connect = format!(
        "<command id=\"connect\">
            <login>{login}</login>
            <password>{password}</password>
            <host>tr1.finam.ru</host>
            <port>3900</port>
        </command>\0"
    );
    let cmd_disconnect = "<command id=\"disconnect\"/>\0";
    let cmd_get_version = "<command id = \"get_connector_version\"/>\0";

    macro_rules! spawn {
        ($f:expr) => {{
            let (tx, rx) = std::sync::mpsc::sync_channel(1 << 10);
            std::thread::spawn(move || {
                for msg in rx.into_iter() {
                    $f(msg)
                }
            });
            tx
        }};
    }

    // 1 отправляет disconnect при "connected=true"
    let tx1 = spawn!(|msg| {
        if matches!(msg, Message::Status(true)) {
            info!("connected");
            std::thread::sleep(std::time::Duration::from_secs(5));
            info!("sending 'disconnect'");
            unsafe { s1.send(cmd_disconnect).unwrap() };
        }
    });

    // 2 отправляет connect при "connected=false"
    let cmd = cmd_connect.clone();
    let tx2 = spawn!(|msg| {
        if matches!(msg, Message::Status(false)) {
            info!("disconnected");
            std::thread::sleep(std::time::Duration::from_secs(5));
            info!("sending 'connect'");
            unsafe { s2.send(cmd.as_bytes()).unwrap() };
        }
    });

    let (tx3, rx3) = std::sync::mpsc::sync_channel(1 << 10);

    // 0 - парсинг и фильтрация сообщений
    let status_version_filter = |buf: TCStr| {
        let bytes = buf.to_bytes();
        if bytes.starts_with(b"<connector_version") {
            // <connector_version>Номер_версии_коннектора</connector_version>
            Some(Message::Version(bytes[19]))
        } else if bytes.starts_with(b"<server_status") {
            // <server_status .. connected="true/false" ..
            let i = bytes.iter().skip(14).position(|b| b'c'.eq(b));
            if i.is_none() {
                // <server_status connected="error"..
                info!("{}", buf.to_string_lossy());
                return None;
            }
            let connected = bytes[i.unwrap() + 14 + 11] == b't';
            Some(Message::Status(connected))
        } else {
            None
        }
    };
    let dispatch = move |msg| {
        let _ = tx1.try_send(msg);
        let _ = tx2.try_send(msg);
        let _ = tx3.try_send(msg);
    };

    txc.input_stream().filter_map(status_version_filter).subscribe(dispatch);

    // start
    unsafe {
        s3.send(cmd_get_version)?;
        s3.send(cmd_connect.as_bytes())?;
    }

    // 3
    let mut prev = std::time::Instant::now();
    let mut n = 0;
    let mut sum = 0usize;
    const N: usize = 5000;

    for _ in rx3.into_iter().filter(|m| matches!(m, Message::Version(_))) {
        sum += (std::time::Instant::now() - prev).as_micros() as usize;
        n += 1;
        if n == N {
            info!("{:0.2} us", sum as f64 / N as f64);
            n = 0;
            sum = 0;
        }
        prev = std::time::Instant::now();
        unsafe { s3.send(cmd_get_version)? };
    }

    Ok(())
}

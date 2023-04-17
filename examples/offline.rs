include!("common/common.rs");

use libtxc::{LogLevel, Stream, TransaqConnector};
use tracing::info;

// запуск примера:
// cargo run --release --example offline
//
// Демонстрaция базового использования библиотеки без необходимости подключения к серверу.
fn main() -> anyhow::Result<()> {
    let (_, _, lib, logdir) = init()?;
    init_logging();

    // Загрузка и инициализация коннектора.
    let mut txc = TransaqConnector::new(lib.into(), logdir.into(), LogLevel::Minimum)?;

    let (tx, rx) = std::sync::mpsc::sync_channel(1 << 10);

    /*
    Создание конвейера обработки входящих сообщений.
    В этом примере поступающие сообщения выводятся в терминал.
    */
    //std::thread::spawn(move || tx.send("".to_string()));

    txc.input_stream().subscribe(move |buf| {
        let _ = tx.send(buf.to_string_lossy().to_string());
    });

    // Создание канала для отправки команд
    let sender = txc.sender();

    let get_version = "<command id = \"get_connector_version\"/>\0";

    info!("Sending 'get_version' command");

    info!("{}", unsafe { sender.send(get_version)? });

    info!("rx: {}", rx.recv().unwrap());

    Ok(())
}

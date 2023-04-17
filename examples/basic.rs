include!("common/common.rs");

use libtxc::{LogLevel, Stream, TransaqConnector};
use tracing::info;

// запуск примера:
// cargo run --release --example basic
//
// Демонстрaция базового использования библиотеки.
fn main() -> anyhow::Result<()> {
    let (login, password, lib, logdir) = init()?;
    init_logging();

    // Загрузка и инициализация коннектора.
    let mut txc = TransaqConnector::new(lib.into(), logdir.into(), LogLevel::Maximum)?;

    /*
    Создание конвейера обработки входящих сообщений.
    В этом примере поступающие сообщения выводятся в терминал.
    */

    txc.input_stream().subscribe(|buf| info!("{buf}"));

    // Создание канала для отправки команд
    let sender = txc.sender();

    // Отправка команды подключения
    let connect = format!(
        "<command id=\"connect\">
            <login>{login}</login>
            <password>{password}</password>
            <host>tr1.finam.ru</host>
            <port>3900</port>
        </command>\0"
    );

    info!("Sending 'connect' command");

    info!("{}", unsafe { sender.send(connect)? });

    // При успешном подключении сервер начнёт отправку чудовищного массива данных,
    // это займёт до 20 сек.
    std::thread::sleep(std::time::Duration::from_secs(20));

    unsafe { sender.send("<command id=\"server_status\"/>\0")? };

    std::thread::sleep(std::time::Duration::from_secs(2));

    unsafe { sender.send("<command id=\"disconnect\"/>\0")? };

    Ok(())
}

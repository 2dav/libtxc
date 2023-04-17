include!("common/common.rs");

use libtxc::{LogLevel, Stream, TransaqConnector};
use tracing::info;

// запуск примера:
// cargo run --release --example input_filter
//
// Использование комбинаторов для фильтрации входящих сообщений
fn main() -> anyhow::Result<()> {
    let (login, password, lib, logdir) = init()?;
    init_logging();

    let mut txc = TransaqConnector::new(lib.into(), logdir.into(), LogLevel::Minimum)?;

    // Обьект, возвращённый `TransaqConnector::input_stream()`, реализует `libtxc::Stream`,
    // содержащий методы для компоновки конвейера обработки входящих сoобщений, этот пример
    // демонстрирует использование комбинаторов `map` и `filter` для фильтрации на основе xml тэга.
    //
    // см. `libtxc::Stream` для списка доступных комбинаторов.

    let is_result = |msg: &str| msg.starts_with("<result");
    let is_error = |msg: &str| msg.starts_with("<error");
    let is_server_status = |msg: &str| msg.starts_with("<server_status");

    txc.input_stream()
        .map(|buf| buf.to_string_lossy().to_string())
        .filter(move |msg| is_result(msg) || is_error(msg) || is_server_status(msg))
        .subscribe(|msg| info!("{msg}"));

    unsafe {
        txc.sender().send(format!(
            "<command id=\"connect\">
                <login>{login}</login>
                <password>{password}</password>
                <host>tr1.finam.ru</host>
                <port>3900</port>
            </command>\0"
        ))?
    };

    std::thread::sleep(std::time::Duration::from_secs(20));

    Ok(())
}

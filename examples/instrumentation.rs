include!("common/common.rs");

use libtxc::{LogLevel, Stream, TransaqConnector};
use tracing_subscriber::layer::SubscriberExt;

// запуск примера:
// > *run tracy*
// > cargo run --release --example instrumentation --features "tracing"
//
// Использование средств инструментации `tokio-rs/tracing`.
//
// Код участков представляющих интерес для профайлинга оснащён tracing-probes,
// в этом примере метрики отправляются в профайлер `wolfpld/tracy`.
//
// Перед запуском примере запустите профайлер 'tracy' и нажмите кнопку 'Connect' в GUI.
fn main() -> anyhow::Result<()> {
    let (login, password, lib, logdir) = init()?;

    // Инициализация бекэнда отправки метрик в 'tracy'
    // см. https://docs.rs/tracing/latest/tracing для описания возможностей 'tracing'
    tracing::subscriber::set_global_default(
        tracing_subscriber::registry().with(tracing_tracy::TracyLayer::new()),
    )?;

    let mut txc = TransaqConnector::new(lib.into(), logdir.into(), LogLevel::Minimum)?;

    txc.input_stream().subscribe(|buf| println!("{buf}"));

    let sender = txc.sender();
    let connect = format!(
        r#"
    <command id="connect">
        <login>{login}</login>
        <password>{password}</password>
        <host>tr1.finam.ru</host>
        <port>3900</port>
    </command>"#,
    );

    unsafe { sender.send(connect) }?;
    // на данном этапе 'tracy' начнёт получать метрики и обновлять GUI.
    std::thread::sleep(std::time::Duration::from_secs(20));
    unsafe { sender.send("<command id=\"disconnect\"/>") }?;

    Ok(())
}

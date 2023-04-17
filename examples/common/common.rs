use anyhow::Context;
use dotenvy::dotenv;

#[allow(unused)]
fn init_logging() {
    // init logging
    tracing_subscriber::fmt().init();
}

#[allow(unused)]
fn init_instrumentation() {
    use tracing_subscriber::layer::SubscriberExt;
    tracing::subscriber::set_global_default(
        tracing_subscriber::registry().with(tracing_tracy::TracyLayer::new()),
    )
    .expect("set up the subscriber");
}

pub fn init() -> anyhow::Result<(String, String, String, String)> {
    // read parameters from '.env'
    const K_LOGIN: &str = "TXC_LOGIN";
    const K_PASSWD: &str = "TXC_PASSWORD";
    const K_LIB: &str = "TXC_LIB";
    const K_LOG_DIR: &str = "TXC_LOG_DIR";
    let no_env_msg = || {
        format!(
            "\nДля запуска любого из примеров создайте '.env' файл в корневой директории проекта \
            со следующим содержанием:\n\
            {} = 'логин сервиса Transaq Connector'\n\
            {} = 'пароль'\n\
            {} = 'путь к txmlconnector64.dll или txcn64.dll'\n\
            {} = 'путь к директории для логов коннектора'",
            K_LOGIN, K_PASSWD, K_LIB, K_LOG_DIR
        )
    };

    let env_file = dotenv().with_context(no_env_msg)?;
    macro_rules! read_keys {
        ($map:expr, $($key:expr),+) => {
            Ok(($($map.remove($key).ok_or(anyhow::anyhow!("'{}' не найден в  {env_file:?}", $key))?),+))
        };
    }
    read_keys!(
        std::env::vars().collect::<std::collections::HashMap<String, String>>(),
        K_LOGIN,
        K_PASSWD,
        K_LIB,
        K_LOG_DIR
    )
}

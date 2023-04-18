[TRANSAQ XML Connector](https://www.finam.ru/howtotrade/tconnector/) API для Rust.

`libtxc` позволяет использовать коннектор в программах на `Rust` и добавляет необходимые гарантии 
безопасности.

- [Документация](https://docs.rs/libtxc/latest/)
- [Примеры](https://github.com/2dav/libtxc/tree/master/examples)

# Quickstart
*Cargo.toml*
```toml
[dependencies]
libtxc = "0.2"
```
*main.rs*
```rust
use libtxc::{TransaqConnector, LogLevel};
use std::{error::Error, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>>{
    let lib: PathBuf = /*txcn64.dll / txmlconnector64.dll path*/;
    let logdir: PathBuf = /*logs directory*/;
    let loglevel:LogLevel = LogLevel::Minimum;
    
    let mut txc = TransaqConnector::new(lib.into(), logdir.into(), loglevel)?;
    
    txc.subscribe(|buf| println!("rx: {buf}"));
    txc.sender().send("<command id = \"get_connector_version\"/>\0")?;
    
    std::thread::sleep(std::time::Duration::from_secs(1));
}
```

# License
MIT OR Apache-2.0

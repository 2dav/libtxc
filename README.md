<div align="center">
	<h1>libtxc</h1>
	<p><a href="https://www.finam.ru/howtotrade/tconnector">TRANSAQ XML Connector</a> API для Rust</p>

[crates.io]: https://crates.io/crates/libtxc
[libs.rs]: https://lib.rs/crates/libtxc
[documentation]: https://docs.rs/libtxc
[license]: https://github.com/2dav/libtxc/blob/main/LICENSE

[![crates.io](https://img.shields.io/crates/v/libtxc)][crates.io]
[![libs.rs](https://img.shields.io/badge/libs.rs-libtxc-orange)][libs.rs]
[![documentation](https://img.shields.io/docsrs/libtxc)][documentation]
[![license](https://img.shields.io/crates/l/libtxc)][license]

</div>

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
    unsafe{ txc.sender().send("<command id = \"get_connector_version\"/>\0")? };
    
    std::thread::sleep(std::time::Duration::from_secs(1));
}
```

# License
MIT or Apache-2.0

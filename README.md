# libtxc

Rust интерфейс к [TRANSAQ XML Connector](https://www.finam.ru/howtotrade/tconnector/)

Релизует минимум необходимого для работы с коннектором из rust:
- динамическая загрузкa экземпляров библиотеки
- конвертация `Rust String` <> `C-String`
- автоматическое освобождениe буферов коннектора

### Документация
```bash
cargo doc --no-deps --open
```

### Пример подключения
Скопируйте txmlconnector(64).dll в директорию с репозиторем.
Отредактируйте src/main.rs введите свои логин и пароль. 
##### Сборка примера для Windows
``` bash
cargo run --release
```
##### Кросс-компиляция под Linux и запуск под wine
```bash
make 64
wine target/x86_64-pc-windows-gnu/release/libtxc.exe
```

### Examples
```rust
use libtxc::{LogLevel, LibTxc};
use std::env;
use anyhow::Result;
fn main() -> Result<()>{
    // загрузить библиотеку
    let mut lib: LibTxc = Default::default();
    // инициализировать в текущей директории с минимальным уровнем логирования
    lib.initialize(env::current_dir()?, LogLevel::Minimum)?;
    // установить обработчик
    lib.set_callback(|_|{});
    // отправить команду
    lib.send_command("")?;
    Ok(())
}
// Деструктор Drop::drop для LibTxc вызывает dll::UnInitialize() для корректной остановки
// коннектора и выгружает экземпляр библиотеки
```
Загрузка экземпляра библиотеки
```rust
use libtxc::LibTxc;
use std::env;
use anyhow::Result;

fn main() -> Result<()>{
    // Загрузить txmlconnector(64).dll из текущей директории
    let lib: LibTxc = Default::default();
    // аналогично
    let dll_search_dir = env::current_dir()?;
    let lib = LibTxc::new(dll_search_dir)?;
    Ok(())
}
```
Установка обработчика входящих сообщений
```rust
use libtxc::{LibTxc, TxcBuff};

let mut lib:LibTxc = Default::default();
lib.set_callback(|buff:TxcBuff| {});
```
Обработка сообщений
```rust
use std::ffi::CStr;
use std::ops::Deref;
use libtxc::TxcBuff;

let cb = |buff: TxcBuff|{
    // выделяет память, копирует байты текста, проверяет соответствие utf-8
    let msg: String = buff.into();
};
let cb = |buff:TxcBuff|{
    // raw bytes
    let msg: &[u8] = *buff;
};
```

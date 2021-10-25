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
Отредактируйте src/example.rs введите свои логин и пароль. 
##### Сборка примера для Windows
``` bash
cargo run --release
```
##### Кросс-компиляция под Linux и запуск под wine
```bash
make 64
wine target/x86_64-pc-windows-gnu/release/example.exe
```
### Proxy
Команда сборки (cargo build, make ..) так же собирает прокси сервер,
позволяющий изолировать работу с библиотекой, например, для работы с коннектором из под linux/wine.

Запуск прокси `wine proxy.exe [PORT]`. Значение PORT по-умолчанию 5555.

Пример клиентского приложения `python proxy_client.py`.

Для каждого подключения на основной порт(`command port`) сервер инициализирует экземпляр библиотеки, отправляет
клиенту номер порта для приёма асинхронных сообщений коннектора(`data port`) и ожидает
подключение на этом порту. Цикл приёма-отправки начинается после
подключения на `data port`. 

Данные полученные на `command port` передаютя в команду коннектора `send_command()`
- все сообщения должны иметь завершающий `\0` байт
- ответ коннектора передаётся клиенту на `command port`
- aсинхронные сообщения коннектора передаются на `data port` без
завершающего `\0`.

См. так же прокси-сервер на `C` Артёма Новикова [TXCProxy](https://github.com/novikovag/TXCProxy)

### Rust API
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

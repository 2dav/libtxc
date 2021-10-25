# libtxc

Rust интерфейс к [TRANSAQ XML Connector](https://www.finam.ru/howtotrade/tconnector/)

Релизует минимум необходимого для работы с коннектором из rust:
- динамическая загрузкa экземпляров библиотеки
- конвертация `Rust String` <> `C-String`
- автоматическое освобождениe буферов коннектора

и прокси сервер для работы с библиотекой под linux\wine.

### Документация
```bash
cargo doc --no-deps --open
```

### Пример подключения
Скопируйте txmlconnector(64).dll в директорию с репозиторем.
Отредактируйте src/example.rs введите свои логин и пароль. 

Запуск примерa под Windows
``` bash
cargo run --release --bin example
```
Кросс-компиляция и запуск под wine
```bash
make 64
wine target/x86_64-pc-windows-gnu/release/example.exe
```
### Proxy
Команда сборки (cargo build, make ..) также собирает TCP/IP прокси сервер,
позволяющий изолировать работу с библиотекой, например, для работы с коннектором из под linux/wine.

Запуск под Linux `wine proxy.exe [PORT=5555]`.

Пример клиентского приложения `python proxy_client.py`.

Для каждого подключения на основной порт(`command port`) сервер инициализирует экземпляр библиотеки, отправляет
клиенту номер порта для приёма асинхронных сообщений коннектора(`data port`) и ожидает
подключение на этом порту. Цикл приёма-отправки начинается после подключения на `data port`. 

Данные, полученные на `command port` передаютя в команду коннектора `send_command()`, ответ коннектора передаётся клиенту на `command port`.
- сообщения должны заканчиваться `\0` байтом
- aсинхронные сообщения коннектора передаются на `data port` без
завершающего `\0`

См. также прокси-сервер на `C` Артёма Новикова [TXCProxy](https://github.com/novikovag/TXCProxy)

### Rust API
```rust
use libtxc::{LogLevel, LibTxc};
use std::env;

// загрузить библиотеку
let mut lib: LibTxc = Default::default();
// инициализировать в текущей директории с минимальным уровнем логирования
lib.initialize(env::current_dir()?, LogLevel::Minimum)?;
// установить обработчик
lib.set_callback(|_|{});
// отправить команду
lib.send_command("")?;
// Деструктор Drop::drop для LibTxc вызывает dll::UnInitialize() для корректной остановки
// коннектора и выгружает экземпляр библиотеки
drop(lib);
```
##### Загрузка экземпляра библиотеки
Конструктор `LibTxc::new` принимает аргументом путь к директории в которой
находится txmlconnector(64).dll.

Название файлa библиотеки
- для i686   - `txmlconnector.dll`
- для x86_64 - `txmlconnector64.dll`

```rust
use libtxc::LibTxc;
use std::env;

// Загрузить txmlconnector(64).dll из текущей директории
let dll_search_dir = env::current_dir()?;
let lib = LibTxc::new(dll_search_dir)?;
// аналогично
let lib: LibTxc = Default::default();
```

##### Установка обработчика входящих сообщений
```rust
use libtxc::{LibTxc, TxcBuff};

let mut lib:LibTxc = Default::default();
lib.set_callback(|buff:TxcBuff| {});
```

##### Отправка сообщений
- `LibTxc::send_command()` - отправить rust string-like что-нибудь; копирует данные, добавляет заверщающий \0
- `LibTxc::send_bytes()` - отправить голые байты заканчивающиеся \0; 0-cost

```rust
let lib = ...
lib.send_command("...");
lib.send_bytes("...\0".as_bytes());
```

##### Обработка сообщений
`TxcBuff` передаётся в пользовательскую функцию обратного вызова, содержит указатель на буфер
возвращённый коннектором, позволяет прочитать содержимое.

Освобождение бyфера коннектора(dll:FreeMemory) происходит вместе с деструктором(Drop::drop) `TxcBuff`.

Доступ к содержимому буфера:
- `TxcBuff::deref()`  - получить `[u8]`; 0-cost
- `TxcBuff::as_ref()` - получить `CStr`; 0-cost
- `TxcBuff::into()`   - получить `String`; alloc, memcopy, utf-8 check

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
    let msg: &[u8] = &*buff;
};
```

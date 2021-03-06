#![warn(rustdoc::broken_intra_doc_links)]
#![warn(missing_docs)]
#![warn(rustdoc::missing_crate_level_docs)]
#![warn(rustdoc::invalid_html_tags)]

//! Rust интерфейс к [TRANSAQ XML Connector](https://www.finam.ru/howtotrade/tconnector/)
//!
//! Реaлизует минимум необходимого для работы с коннектором из rust:
//! - динамическая загрузкa экземпляров библиотеки
//! - конвертация `Rust String` <> `C-String`
//! - автоматическое освобождениe буферов коннектора
//! - корректное отключение, остановка коннектора и освобождение ресурсов
//!
//!
//!
//! ##### Загрузка экземпляра библиотеки
//! Конструктор [`LibTxc::new`] принимает аргументом путь к директории в которой
//! находится txmlconnector(64).dll и необязательный параметр экземпляр [slog](https://docs.rs/slog/latest/slog) логгера.
//!
//! Название файлa библиотеки
//! - для i686   - `txmlconnector.dll`
//! - для x86_64 - `txmlconnector64.dll`
//!
//! ```
//! use libtxc::LibTxc;
//! use std::env;
//!
//! fn main() -> Result<()>{
//!     // Загрузить txmlconnector(64).dll из текущей директории
//!     let dll_search_dir = env::current_dir()?;
//!     let lib = LibTxc::new(dll_search_dir, None)?;
//!     // аналогично
//!     let lib: LibTxc = Default::default();
//!     Ok(())
//! }
//! ```
//!
//! ##### Установка обработчика входящих сообщений
//! см. [`LibTxc::set_callback()`]
//! ```
//! use libtxc::{LibTxc, TxcBuff};
//!
//! let mut lib:LibTxc = Default::default();
//! lib.set_callback(|buff:TxcBuff| {});
//! ```
//! ##### Отправка сообщений
//! - [`LibTxc::send_command()`] - отправить string-like что-нибудь; копирует данные, добавляет заверщающий \0
//! - [`LibTxc::send_bytes()`] - отправить голые байты заканчивающиеся \0
//! ```
//! let lib = ...
//! lib.send_command("...");
//! lib.send_bytes("...\0".as_bytes());
//! ```
//! ##### Обработка сообщений
//! [`TxcBuff`] передаётся в пользовательскую функцию обратного вызова, содержит указатель на буфер
//! возвращённый коннектором, позволяет прочитать содержимое.
//!
//! Освобождение бyфера коннектора(dll:FreeMemory) происходит вместе с деструктором(Drop::drop) `TxcBuff`.
//!
//! Доступ к содержимому буфера:
//! - [`TxcBuff::deref()`]  - получить `[u8]`
//! - [`TxcBuff::as_ref()`] - получить [`CStr`]
//! - [`Into::into()`]   - получить [`String`]; выделяет память, копирует байты текста, проверяет соответствие utf-8
//! ```
//! use std::ffi::CStr;
//! use std::ops::Deref;
//! use libtxc::TxcBuff;
//!
//! lib.set_callback(|buff:TxcBuff| {
//!     let msg: String = buff.into();
//!     let msg: CStr = buff.as_ref();
//!     let msg: &[u8] = &*buff;
//! });
//! ```

mod ffi;

use slog::{error, info, o, trace, warn, Drain};
use std::ffi::CStr;
use std::fmt::Display;
use std::ops::Deref;
use std::os::raw::c_int;
use std::result::Result;
use std::{env, fmt, path::PathBuf};

/// Ошибки вызовов библиотеки
#[derive(Debug)]
pub struct Error {
    /// функция библиотеки
    pub method: String,
    /// аргументы
    pub args: Option<String>,
    /// текст сообщения об ошибке
    pub message: String,
}

impl From<Error> for std::io::Error {
    fn from(e: Error) -> Self {
        std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "txc.dll::{}({:?}) -> {}", self.method, self.args, self.message)
    }
}

// counts the number of expanded elements, compiles to const expression
macro_rules! count {
    () => (0usize);
    ( $x:tt $($xs:tt)* ) => (1usize + count!($($xs)*));
}
// helper simplifying `Error` creation with varying arguments
macro_rules! egeneric {
    ($method:expr, $msg:expr) => {
        egeneric!($method, None, $msg)
    };
    ($method:expr, [$($args:ident),*], $msg:expr) => {{
        let mut name_value = Vec::<String>::with_capacity(count!($($args)*));
        $(name_value.push(format!("{}: {:?}", stringify!($args), $args));)*
        egeneric!($method, Some(name_value.join(", ")), $msg)
    }};
    ($method:expr, $args:expr, $msg:expr) => {
        Error {
            method: format!("{}", $method),
            args: $args,
            message: format!("{}", $msg),
        }
    };
}

/// Глубина логирования в соответствии с детализацией и размером лог-файла
#[derive(Debug, Clone, Copy)]
#[repr(i32)]
pub enum LogLevel {
    /// минимальный
    Minimum = 1,
    /// стандартный(по-умолчанию)
    Default = 2,
    /// максимальный
    Maximum = 3,
}

impl Default for LogLevel {
    fn default() -> Self {
        LogLevel::Default
    }
}

impl From<u8> for LogLevel {
    fn from(me: u8) -> LogLevel {
        match me {
            1 => LogLevel::Minimum,
            3 => LogLevel::Maximum,
            _ => LogLevel::Default,
        }
    }
}

/// Интерфейс к коннектору.
///
/// Содержит экземпляр динамически загруженной библиотеки.
/// - `!Sync` + `!Send` не может быть передан между потоками
/// - остановка коннектора, выгрузка библиотеки и освобождение ресурсов происходят в деструкторе [`Drop`]
pub struct LibTxc {
    imp: ffi::Lib,
    log: slog::Logger,
}

impl Default for LibTxc {
    fn default() -> Self {
        LibTxc::new(env::current_dir().unwrap(), None).unwrap()
    }
}

impl Drop for LibTxc {
    #[inline]
    fn drop(&mut self) {
        if let Err(msg) = self.uninitialize() {
            error!(self.log, "{}", msg);
        }
    }
}

/// Обертка над буфером Transaq Connector.
///
/// Передаётся в пользовательскую функцию обратного вызова, содержит указатель на буфер
/// возвращённый коннектором.
///
/// Освобождение бyфера коннектора(dll:FreeMemory) происходит вместе с деструктором(Drop::drop) `TxcBuff`.
///
/// Доступ к содержимому буфера:
/// - [`TxcBuff::deref()`]  - получить `[u8]`
/// - [`TxcBuff::as_ref()`] - получить `CStr`
/// - [`Into::into()`]   - получить `String`; выделяет память, копирует байты текста, проверяет соответствие utf-8
///
///
/// # Примеры
/// ```
/// use std::ffi::CStr;
/// use std::ops::Deref;
/// use libtxc::TxcBuff;
///
/// let buff: TxcBuff = ...;
/// // выделяет память, копирует байты текста, проверяет соответствие utf-8
/// let msg: String = buff.into();
/// // CStr
/// let msg: &[u8] = buff.as_ref();
/// // raw bytes
/// let msg: &[u8] = &*buff;
/// ```
pub struct TxcBuff<'a>(*const u8, &'a LibTxc);

impl Drop for TxcBuff<'_> {
    #[inline]
    fn drop(&mut self) {
        trace!(self.1.log, "txc::free_memory");

        #[cfg(feature = "unchecked")]
        self.1.imp.free_memory(self.0);

        #[cfg(not(feature = "unchecked"))]
        if !self.1.imp.free_memory(self.0) {
            // FreeMemory() == false с живым буфером недокументированная ситуация
            warn!(self.1.log, "Операция очистки txc буфера FreeMemory(*) завершилась неудачно.");
        }
    }
}

impl AsRef<CStr> for TxcBuff<'_> {
    #[inline]
    fn as_ref(&self) -> &CStr {
        unsafe { CStr::from_ptr(self.0.cast()) }
    }
}

impl Deref for TxcBuff<'_> {
    type Target = [u8];

    #[inline]
    /// Получить ссылку на содержимое буфера без завершающего \0.
    fn deref(&self) -> &Self::Target {
        self.as_ref().to_bytes()
    }
}

impl From<TxcBuff<'_>> for String {
    #[inline]
    fn from(buff: TxcBuff) -> Self {
        #[cfg(not(feature = "unchecked"))]
        {
            // validates utf-8 conformance
            buff.as_ref().to_string_lossy().to_string()
        }
        #[cfg(feature = "unchecked")]
        {
            // assumes bytes are valid utf-8
            unsafe { String::from_utf8_unchecked(buff.deref().to_vec()) }
        }
    }
}

impl Display for TxcBuff<'_> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_ref().to_string_lossy())
    }
}

// composes full path to the library according to the target platform
fn lib_path(mut dir: PathBuf) -> Result<PathBuf, std::io::Error> {
    #[cfg(target_arch = "x86")]
    dir.push("txmlconnector");
    #[cfg(target_arch = "x86_64")]
    dir.push("txmlconnector64");
    dir.set_extension("dll");
    if dir.exists() {
        Ok(dir)
    } else {
        let msg = format!("file {:?} do not exists", dir);
        Err(std::io::Error::new(std::io::ErrorKind::NotFound, msg))
    }
}

impl LibTxc {
    /// Загружает библиотеку в пространство текущего процесса
    /// * `dll_dir` - путь к директории в которой находится txmlconnector(64).dll
    /// * `log`     - экземпляр [slog](https://docs.rs/slog/latest/slog) логгера, необязательный
    /// параметр
    ///
    /// Название файлa библиотеки
    /// - для i686   - `txmlconnector.dll`
    /// - для x86_64 - `txmlconnector64.dll`
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// use libtxc::LibTxc;
    ///
    /// let lib: LibTxc = Default::default();
    /// // аналогично
    /// use std::env;
    /// let dll_search_dir = env::current_dir().unwrap();
    /// let lib = LibTxc::new(dll_search_dir, None).unwrap();
    /// ```
    /// ```
    /// use libtxc::LibTxc;
    /// use std::path::PathBuf;
    ///
    /// let dll_search_dir:PathBuf = ["path", "to", "txmlconnector_dll", "directory"].iter().collect();
    /// let lib = LibTxc::new(dll_search_dir, None).unwrap();
    /// ```
    pub fn new<L: Into<Option<slog::Logger>>>(
        dll_dir: PathBuf,
        log: L,
    ) -> Result<Self, std::io::Error> {
        let log =
            log.into().unwrap_or_else(|| slog::Logger::root(slog_stdlog::StdLog.fuse(), o!()));
        lib_path(dll_dir)
            .and_then(|path| {
                info!(log, "Loading library {}", path.to_str().unwrap());
                unsafe { ffi::load(path) }
            })
            .map(|imp| LibTxc { imp, log })
    }

    /// Bыполняет инициализацию библиотеки: запускает поток обработки очереди
    /// обратных вызовов, инициализирует систему логирования библиотеки.
    ///
    /// * `log_path`  - Путь к директории, в которую будут сохраняться файлы отчетов (XDF*.log, DSP*.txt, TS*.log)
    /// * `log_level` - Глубина логирования
    ///
    /// Функция `initialize` может быть вызвана в процессе работы с Коннектором
    /// повторно для изменения директории и уровня логирования, но только в
    /// случае, когда библиотека остановлена, то есть была выполнена команда
    /// disconnect или соединение еще не было установлено.
    ///
    /// Функция должна быть выполнена перед началом работы с библиотекой, то есть перед
    /// первой отправкой команды.
    /// Каждый успешный вызов функции `initialize` должен сопровождаться вызовом
    /// функции [`LibTxc::uninitialize()`].
    ///
    ///
    /// # Examples
    ///
    /// ```
    /// use std::env;
    /// use libtxc::LibTxc;
    /// use anyhow::Result;
    ///
    /// fn main() ->Result<()> {
    ///     let lib:LibTxc = Default::default();
    ///     let log_path = env::current_dir()?;
    ///     lib.initialize(log_path, Default::default())?;
    ///     Ok(())
    /// }
    /// ```
    /// # Errors
    ///
    /// [`Error`] ошибкa, возвращённая библиотекой
    pub fn initialize(&mut self, log_path: PathBuf, log_level: LogLevel) -> Result<(), Error> {
        if !log_path.exists() {
            return Err(egeneric!(
                "Initialize",
                [log_path],
                "директория не существует или недоступна"
            ));
        }

        let c_log_path = ffi::to_cstring(log_path.display().to_string());

        trace!(self.log, "txc::initialize");
        self.imp
            .initialize(c_log_path.as_c_str(), log_level as c_int)
            .map_err(|msg| egeneric!("Initialize", [log_path, log_level], msg))
    }

    /// Выполняет остановку внутренних потоков библиотеки, в том числе завершает
    /// поток обработки очереди обратных вызовов. Останавливает систему
    /// логирования библиотеки и закрывает файлы отчетов.
    ///
    /// Функция вызывается автоматически в момент окончания жизни экземпляра `LibTxc`
    ///
    ///
    /// # Errors
    ///
    /// [`Error`] ошибкa, возвращённая библиотекой
    pub fn uninitialize(&self) -> Result<(), Error> {
        trace!(self.log, "txc::uninitialize");
        self.imp.uninitialize().map_err(|msg| egeneric!("UnInitialize", msg))
    }

    /// Изменяет уровень логирования без остановки библиотеки.
    ///
    ///
    /// # Errors
    ///
    /// [`Error`] ошибкa, возвращённая библиотекой
    pub fn set_loglevel(&self, log_level: LogLevel) -> Result<(), Error> {
        trace!(self.log, "txc::set_log_level");
        self.imp
            .set_log_level(log_level as c_int)
            .map_err(|msg| egeneric!("SetLogLevel", [log_level], msg))
    }

    /// Служит для передачи команд Коннектору.
    ///
    /// * `command` - Указатель на строку, содержащую xml команду для библиотеки TXmlConnector
    ///
    /// В случае успеха возвращает сообщение коннектора.
    ///
    /// Функция может выполняться только в период между вызовами функций
    /// [`LibTxc::initialize`()] и [`LibTxc::uninitialize()`].
    ///
    ///
    /// # Errors
    ///
    /// [`Error`] ошибкa, возвращённая библиотекой
    pub fn send_command<C: AsRef<str>>(&self, command: C) -> Result<String, Error> {
        self.send_bytes(ffi::to_cstring(command).as_bytes_with_nul())
    }

    /// В отличие от [`LibTxc::send_command()`], принимает байты в качестве аргумента.
    /// Этот метод не имеет затрат связанных с конвертацией Rust String -> C-String, предполагая
    /// что данные уже имеют завершающий \0.
    ///
    /// # Panics
    /// Если последний байт отличаетсся от \0.
    pub fn send_bytes<C: AsRef<[u8]>>(&self, command: C) -> Result<String, Error> {
        let pl = command.as_ref();

        #[cfg(not(feature = "unchecked"))]
        if pl.is_empty() || pl.last().unwrap().ne(&b'\0') {
            let cmd = unsafe { std::str::from_utf8_unchecked(pl) };
            return Err(egeneric!(
                "SendCommand",
                [cmd],
                "Данные для отправки должны иметь завершающий \0"
            ));
        }

        self.imp.send_bytes(pl).map_err(|err| {
            let cmd = unsafe { std::str::from_utf8_unchecked(pl) };
            egeneric!("SendCommand", [cmd], err)
        })
    }

    /// Устанавливает функцию обратного вызова, которая
    /// будет принимать асинхронные информационные сообщения от Коннектора.
    ///
    /// * `callback` - функция обратного вызова
    pub fn set_callback<F>(&self, mut callback: F)
    where
        F: FnMut(TxcBuff),
    {
        trace!(self.log, "txc::set_callback");
        self.imp.set_callback(
            #[inline(always)]
            move |p| callback(TxcBuff(p, self)),
        );
    }
}

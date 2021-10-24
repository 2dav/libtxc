#![warn(rustdoc::broken_intra_doc_links)]
#![warn(missing_docs)]
#![warn(rustdoc::missing_crate_level_docs)]
#![warn(rustdoc::invalid_html_tags)]

//! Rust интерфейс к [TRANSAQ XML Connector](https://www.finam.ru/howtotrade/tconnector/)
//!
//! Релизует минимум необходимого для работы с коннектором из rust:
//! - динамическая загрузкa экземпляров библиотеки
//! - конвертация `Rust String` <> `C-String`
//! - автоматическое освобождениe буферов коннектора
//!
//!
//! ### Examples
//! ```
//! use libtxc::{LogLevel, LibTxc};
//! use std::env;
//! fn main() -> Result<()>{
//!     // загрузить библиотеку
//!     let mut lib: LibTxc = Default::default();
//!     // инициализировать в текущей директории с минимальным уровнем логирования
//!     lib.initialize(env::current_dir()?, LogLevel::Minimum)?;
//!     // установить обработчик
//!     lib.set_callback(|_|{});
//!     // отправить команду
//!     lib.send_command("")?;
//!     Ok(())
//! }
//! // Деструктор Drop::drop для LibTxc вызывает dll::UnInitialize() для корректной остановки
//! // коннектора и выгружает экземпляр библиотеки
//! ```
//! Загрузка экземпляра библиотеки см. [`LibTxc::new()`]
//! ```
//! use libtxc::LibTxc;
//! use std::env;
//!
//! fn main() -> Result<()>{
//!     // Загрузить txmlconnector(64).dll из текущей директории
//!     let lib: LibTxc = Default::default();
//!     // аналогично
//!     let dll_search_dir = env::current_dir()?;
//!     let lib = LibTxc::new(dll_search_dir)?;
//!     Ok(())
//! }
//! ```
//! Установка обработчика входящих сообщений см. [`LibTxc::set_callback()`]
//! ```
//! use libtxc::{LibTxc, TxcBuff};
//!
//! let mut lib:LibTxc = Default::default();
//! lib.set_callback(|buff:TxcBuff| {});
//! ```
//! Обработка сообщений см. [`TxcBuff`]
//! ```
//! use std::ffi::CStr;
//! use std::ops::Deref;
//! use libtxc::TxcBuff;
//!
//! let cb = |buff: TxcBuff|{
//!     // выделяет память, копирует байты текста, проверяет соответствие utf-8
//!     let msg: String = buff.into();
//! };
//! let cb = |buff:TxcBuff|{
//!     // raw bytes
//!     let msg: &[u8] = *buff;
//! };
//! ```

mod ffi;

use std::ffi::CStr;
use std::marker::PhantomData;
use std::ops::Deref;
use std::os::raw::c_int;
use std::result::Result;
use std::{borrow::Cow, env, fmt, path::PathBuf};

/// Ошибки вызовов библиотеки
#[derive(Debug)]
pub struct Error {
    /// метод библиотеки, приведший к ошибке
    method: String,
    /// аргументы
    args: String,
    /// текст сообщения об ошибке, возвращённый библиотекой
    message: String,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "txc.dll::{}({}) -> {}",
            self.method, self.args, self.message
        )
    }
}

macro_rules! egeneric {
    ($method:expr, $msg:expr) => {
        egeneric!($method, [], $msg)
    };
    ($method:expr, [$($args:ident),*], $msg:expr) => {{
        #[allow(unused_mut)]
        let mut name_value = Vec::<String>::new();
        $(name_value.push(format!("{}: {:?}", stringify!($args), $args));)*
        Err(Error {
            method: format!("{}", $method),
            args: name_value.join(", "),
            message: format!("{}", $msg),
        })
    }};
}

/// Глубина логирования в соответствии с детализацией и размером лог-файла
#[derive(Debug, Clone, Copy)]
pub enum LogLevel {
    /// минимальный
    Minimum,
    /// стандартный(по-умолчанию)
    Default,
    /// максимальный
    Maximum,
}

impl Default for LogLevel {
    fn default() -> Self {
        LogLevel::Default
    }
}

impl From<LogLevel> for c_int {
    fn from(me: LogLevel) -> c_int {
        match me {
            LogLevel::Minimum => 1,
            LogLevel::Default => 2,
            LogLevel::Maximum => 3,
        }
    }
}

/// Интерфейс к коннектору.
///
/// Содержит экземпляр динамически загруженной библиотеки и пользовательскую функцию обратного
/// вызова.
/// - `!Sync` + `!Send` не может быть передан между потоками
/// - выгрузка библиотеки и освобождение ресурсов происходят в деструкторе [`Drop`]
pub struct LibTxc {
    imp: ffi::Lib,
    _marker: PhantomData<*const ()>, // !Sync + !Send
}

impl Default for LibTxc {
    fn default() -> Self {
        LibTxc::new(env::current_dir().unwrap()).unwrap()
    }
}

impl Drop for LibTxc {
    #[allow(unused)]
    fn drop(&mut self) {
        self.uninitialize();
    }
}

/// Обертка над буфером Transaq Connector.
///
/// Передаётся в пользовательскую функцию обратного вызова, содержит указатель на буфер
/// возвращённый коннектором, позволяет прочитать содержимое.
///
/// Освобождение бyфера коннектора(dll:FreeMemory) происходит вместе с деструктором(Drop::drop) `TxcBuff`.
///
/// Доступ к содержимому буфера:
/// - получить `&[u8]`  - 0-cost
/// - получить `CStr`   - 0-cost
/// - получить `String` - alloc, memcopy, utf-8 check
///
///
/// # Examples
/// ```
/// use libtxc::TxcBuff;
/// use std::ffi::CStr;
/// use std::ops::Deref;
///
/// let cb = |buff:TxcBuff|{
///     // выделяет память, копирует байты текста, проверяет соответствие utf-8
///     let owned: String = buff.into();
/// };
/// let cb = |buff:TxcBuff|{
///     // работа с содержимым буфера напрямую
///     let  cstr: &CStr = buff.as_ref();
///     let bytes: &[u8] = *buff;
/// };
/// ```
pub struct TxcBuff<'a> {
    lib: &'a ffi::Lib,
    p: *const u8,
}

impl Drop for TxcBuff<'_> {
    fn drop(&mut self) {
        if !self.lib.free_memory(self.p) {
            // bad?
        }
    }
}
impl AsRef<CStr> for TxcBuff<'_> {
    fn as_ref(&self) -> &CStr {
        unsafe { CStr::from_ptr(self.p.cast()) }
    }
}

impl Deref for TxcBuff<'_> {
    type Target = [u8];

    /// Получить ссылку на содержимое буфера без завершающего \0.
    fn deref(&self) -> &Self::Target {
        self.as_ref().to_bytes()
    }
}

impl From<TxcBuff<'_>> for String {
    fn from(buff: TxcBuff) -> Self {
        match buff.as_ref().to_string_lossy() {
            Cow::Owned(m) => m,
            Cow::Borrowed(s) => s.to_owned(),
        }
    }
}

impl LibTxc {
    /// Загружает библиотеку в пространство текущего процесса
    /// * `dll_dir` - путь к директории в которой находится txmlconnector(64).dll
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
    /// let lib = LibTxc::new(dll_search_dir).unwrap();
    /// ```
    /// ```
    /// use libtxc::LibTxc;
    /// use std::path::PathBuf;
    ///
    /// let dll_search_dir:PathBuf = ["path", "to", "txmlconnector_dll", "directory"].iter().collect();
    /// let lib = LibTxc::new(dll_search_dir).unwrap();
    /// ```
    pub fn new(mut dll_dir: PathBuf) -> Result<Self, std::io::Error> {
        #[cfg(target_arch = "x86")]
        dll_dir.push("txmlconnector");
        #[cfg(target_arch = "x86_64")]
        dll_dir.push("txmlconnector64");

        dll_dir.set_extension("dll");
        assert!(dll_dir.exists(), "{:?} not exists", dll_dir);

        let lib = unsafe { ffi::load(dll_dir) }?;
        Ok(LibTxc {
            imp: lib,
            _marker: PhantomData,
        })
    }

    #[inline(always)]
    fn wrap_txc_buffer(&self, p: *const u8) -> TxcBuff {
        TxcBuff { lib: &self.imp, p }
    }

    #[inline(always)]
    fn errmsg(&self, p: *const u8) -> Option<String> {
        if p.is_null() {
            None
        } else {
            Some(self.wrap_txc_buffer(p).into())
        }
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
        assert!(log_path.exists(), "{:?} not exists", log_path);
        let c_log_path = ffi::to_cstring(log_path.display().to_string());
        let r = self.imp.initialize(c_log_path.as_c_str(), log_level.into());
        self.errmsg(r)
            .map(|msg| egeneric!("Initialize", [log_path, log_level], msg))
            .unwrap_or(Ok(()))
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
        self.errmsg(self.imp.uninitialize())
            .map(|msg| egeneric!("UnInitialize", msg))
            .unwrap_or(Ok(()))
    }

    /// Изменяет уровень логирования без остановки библиотеки.
    ///
    ///
    /// # Errors
    ///
    /// [`Error`] ошибкa, возвращённая библиотекой
    pub fn set_loglevel(&self, log_level: LogLevel) -> Result<(), Error> {
        self.errmsg(self.imp.set_log_level(log_level.into()))
            .map(|msg| egeneric!("SetLogLevel", [log_level], msg))
            .unwrap_or(Ok(()))
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
    pub fn send_command(&self, command: &str) -> Result<String, Error> {
        let c_str = ffi::to_cstring(command);
        let r = self.imp.send_command(c_str.as_c_str());
        let msg: String = self.wrap_txc_buffer(r).into();
        if msg.chars().nth(17).unwrap() == 't' {
            // <result success=”true” ... />
            // .................^
            Ok(msg)
        } else {
            // <result success=”false”>
            //  <message>error message</message>
            // </result>
            //
            // <error> Текст сообщения об ошибке</error>
            egeneric!("SendCommand", [command], msg)
        }
    }

    /// Устанавливает функцию обратного вызова, которая
    /// будет принимать асинхронные информационные сообщения от Коннектора.
    ///
    /// * `callback` - функция обратного вызова
    pub fn set_callback<F>(&self, callback: F)
    where
        F: Fn(TxcBuff),
    {
        self.imp.set_callback(
            #[inline(always)]
            move |p| callback(self.wrap_txc_buffer(p)),
        );
    }
}

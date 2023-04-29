#![warn(rustdoc::broken_intra_doc_links)]
#![warn(missing_docs)]
#![warn(rustdoc::missing_crate_level_docs)]
#![warn(rustdoc::invalid_html_tags)]

//! [TRANSAQ XML Connector](https://www.finam.ru/howtotrade/tconnector/) API для Rust.
//!
//!`libtxc` позволяет использовать коннектор в программах на `Rust` и добавляет необходимые гарантии
//! безопасности.
//!
//! В частности, исключено возникновение подвисших указателей на ресурсы библиотеки, наличие
//! [`TCStr`](smart-pointer на ресурсы библиотеки) или [`Sender`](обьект-отправитель
//! сообщений) в любом участке кода гарантирует безопасность связанных с ними операций.
//! Исключён потенциальный *deadlock*, присутствующий в дизайне коннектора.
//!
//! - [Примеры](https://github.com/2dav/libtxc/tree/master/examples)
//!
//! # Features
//! **catch_unwind** *включено по умолчанию*
//!
//! Возникновение паники(*panic*) по умолчанию запускает разматывание стека(*stack unwinding*), и на
//! данный момент это приводит к `undefined behaviour`, если паника произошла в окружении другого языка.
//! *catch_unwind* включает проверку паники в callback-коде; в случае её возникновения выводится
//! сообщение и процесс аварийно завершается.
//!
//! **safe_buffers** *включено по умолчанию*
//!
//! Если предположить возникновение ситуации, при которой коннектор вернёт нулевой указатель, или
//! ответ коннектора будет содержать некорректные данные, это немедленно приведёт к `undefined behaviour`.
//! *safe_buffers* включает проверку указателей и содержимого буферов, возвращённых коннектором.
//!
//! **tracing**
//!
//! `libtxc` содержит [`tracing`](https://docs.rs/tracing/latest/tracing/) "probes", которые могут
//! быть использованы для сбора онлайн-метрик, профилирования пользовательского кода обратного вызова
//! или отладки. Включение опции *tracing* добавляет зависимость `tokio-rs/tracing` и код инструментации.
//!
//! ## License
//! <sup>
//! Licensed under either of <a href="https://github.com/2dav/libtxc/blob/master/LICENSE-APACHE">Apache License, Version
//! 2.0</a> or <a href="https://github.com/2dav/libtxc/blob/master/LICENSE-MIT">MIT license</a> at your option.
//! </sup>
//!
//! <br/>
//!
//! <sub>
//! Unless you explicitly state otherwise, any contribution intentionally submitted
//! for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
//! be dual licensed as above, without any additional terms or conditions.
//! </sub>
#[cfg(not(windows))]
compile_error!(
    "TXC library is a 'Windows DLL', and so this doesn't work on anything but 'MS Windows', sorry"
);

use std::{cell::Cell, fmt, io, path::PathBuf, sync::Arc};
#[cfg(feature = "tracing")]
use tracing::instrument;

mod buffers;
mod callback;
mod ffi;
mod stream;

use buffers::{as_nonnull_txc_buf, parse_send_response};
use callback::{BoxT, InputStream};

pub use buffers::TCStr;
pub use stream::Stream;

/// Перечисление возможных ошибок и исключительных ситуаций
#[derive(Debug)]
pub enum Error {
    /// Ошибка, возникшая во время загрузки библиотеки
    Loading(io::Error),
    /// Ошибка инициализации TransaqXMLConnector
    Initialization(String),
    /// Ошибка обработки команды
    InvalidCommand(String),
    /// Внутренняя ошибка/исключение коннектора
    Internal(String),
}

#[allow(missing_docs)]
pub type Result<T = ()> = std::result::Result<T, Error>;

struct Inner {
    callback: Cell<Option<BoxT>>,
    module: ffi::Module,
}
// 'TransaqConnector' is non-`Copy`, and it is the only one who might mutate `Inner`,
// see `TransaqConnector::input_stream` for soundness of this.
unsafe impl Sync for Inner {}

/// Экземпляр загруженной библиотеки
///
/// `TransaqConnector` содержит экземпляр динамически загруженной библиотеки и предоставляет
/// интерфейс к основным функциям:
/// - [`TransaqConnector::input_stream()`] - для обработки входящих сообщений,
/// - [`TransaqConnector::sender()`] - для отправки сообщений
///
/// Время жизни библиотеки(dll модуля) определяется счётчиком ссылок, который разделяется между всеми
/// экземплярами [`Sender`] и [`TransaqConnector`]. Остановка коннектора и освобождение ресурсов
/// происходят в момент удаления последней ссылки.
///
/// `TransaqConnector` существует в единственном экземпляре, но владение может быть передано между
/// потоками.
#[repr(transparent)]
pub struct TransaqConnector(Arc<Inner>);
unsafe impl Send for TransaqConnector {}

impl TransaqConnector {
    /// Загружает и подготавливает библиотеку к использованию
    ///
    /// Загружает динамическую библиотеку, расположенную по пути **library_path** при помощи API
    /// ОС.
    ///
    /// Инициализирует библиотеку `txc::initialize(3)` с директорией для логов коннектора **log_dir**
    /// и уровнем логирования **logging_level**.
    ///
    /// # Errors
    /// - [`Error::Loading`] - библиотека не найдена по указанному пути, ошибка API ОС во время загрузки,
    /// попытка повторной загрузки библиотеки
    /// - [`Error::Initialization`] - внутренняя ошибка коннектора во время инициализации
    pub fn new(library_path: PathBuf, log_dir: PathBuf, logging_level: LogLevel) -> Result<Self> {
        if !library_path.exists() {
            let msg = format!("file {library_path:?} do not exists");
            return Err(Error::Loading(io::Error::new(io::ErrorKind::NotFound, msg)));
        }

        let module = unsafe { ffi::Module::load(library_path).map_err(Error::Loading)? };

        module.initialize(log_dir, logging_level as _).map_err(Error::Initialization)?;

        Ok(Self(Arc::new(Inner { module, callback: Cell::new(None) })))
    }

    /// Создаёт обьект-отправитель сообщений
    ///
    /// `Sender` содержит жёсткую ссылку(`strong reference`) на экземпляр загруженной библиотеки,
    /// что препятствует её выгрузке, пока существует хотя бы один экземпляр `Sender`.   
    ///
    /// Создание `Sender` является легковесной операцией лишь увеличивающей счётчик ссылок.
    /// Однако, создание нового `Sender` для отправки каждой новой команды приводит к увеличению
    /// трафика между ядрами процессора, что может негативно сказаться на произодительности.  
    ///
    /// # Пример
    /// ```no_run
    /// use libtxc::{TransaqConnector, Sender};
    ///
    /// let txc: TransaqConnector = /*..*/;
    /// let sender: Sender = txc.sender();
    /// // Освобождение `TransaqConnector` не приводит к выгрузке библиотеки, т.к. `sender` всё ещё
    /// // находится в области видимости
    /// drop(txc);
    ///
    /// sender.send("<command id=\"get_connector_version\"/>\0");
    /// ```
    pub fn sender(&self) -> Sender {
        Sender::new(Arc::clone(&self.0))
    }

    /// Создаёт [`Stream`] для компоновки конвейера обработки входящих сообщений
    ///
    /// Вызов [`Stream::subscribe`] регистрирует функцию обратного вызова в качестве
    /// обработчика входящих сообщений.
    ///
    /// Пример минимального обработчкиа
    /// ```no_run
    /// use libtxc::TCStr;
    ///
    /// let mut txc = /*..*/;
    /// txc.input_stream().subscribe(|buf:TCStr| println!("{buf}"));
    /// ```
    ///
    /// При поступлении новых сообщений функция обратного вызова запускается в отдельном потоке,
    /// который управляется библиотекой. В связи с этим, "callback" и его зависимости
    /// должны удовлетворять [`Send`] и [`Sync`], гарантирующим безопасность в многопоточном
    /// окружении
    /// ```no_run
    /// let mut txc = /*..*/;
    /// let v = vec![];
    /// txc.input_stream().subscribe(move |buf|{
    ///     v.extend(buf.to_bytes());
    /// });
    /// println!("{:?}", v);
    /// // Ошибка компиляции! `Vec` требует явной синхронизации для многопоточного использования
    /// ```
    ///
    /// Обработчик может быть установлен повторно в любой момент исполнения программы.
    /// Повторный вызов [`Stream::subscribe`] освобождает ресурсы текущего обработчика и
    /// регистрирует новый; эта операция потоко-безопасна и не требует доп. синхронизации.
    ///
    /// Архитектура коннектора предполагает использование каналов\очередей для передачи сообщений
    /// между потоком данных `TransaqXMLConnector` и обработчиками на других потоках
    /// ```no_run
    /// let mut txc = /*..*/;
    ///
    /// let (tx, rx) = std::sync::mpsc::sync_channel(1<<10);
    ///
    /// txc.input_stream()
    ///     .map(|buf| /*parse(buf)*/)
    ///     .subscribe(move |msg| {
    ///         tx.send(msg);
    ///     });
    ///
    /// // ...
    ///
    /// for msg in rx.into_iter(){
    ///     /* msg processing */
    /// }
    /// ```
    ///
    /// См. [examples](https://github.com/2dav/libtxc/tree/master/examples) в репозитории проекта, для
    /// различных примеров использования.  
    #[inline(always)]
    pub fn input_stream(&mut self) -> impl stream::Stream<Output = TCStr<'_>> + '_ {
        let subscribe_fn = |trampoline: ffi::CallbackEx, payload: BoxT| {
            // `set_callback_ex` and callback execution routine are both internally ordered by the same
            // 'mutex' and this prevents 'race condition' in this section.
            // However further we are mutating internal field without any synchronization, and this
            // is if the compiler/CPU decides to reorder instructions, may cause the current `callback`
            // state to be dropped while it is executing on another thread.
            // To prevent this we need to fix instruction order
            if self.0.module.set_callback_ex(trampoline, payload.as_raw_ptr()) {
                // fix instruction order, see comment above
                unsafe { std::arch::asm!("mfence", options(nostack, preserves_flags)) };
                self.0.callback.set(Some(payload));
            } else {
                eprintln!("`set_callback_ex` - Не удалось установить функцию обратного вызова. \
                В документации к коннектору нет описания этой ситуации, как и способов её исправления.\
                Если вам удалось добиться воспроизводимости этой ошибки создайте issue на github");
            }
        };

        let free_mem = self.0.module.free_memory;
        InputStream(subscribe_fn).map(move |ptr| TCStr::new(ptr, free_mem))
    }
}

/// Обьект-отправитель сообщений.
///
/// Использование методов [`Sender::send`] и [`Sender::send_ptr`] компилируется в прямые вызовы функции  
/// `BYTE* send_command(BYTE*)` коннектора.
///
/// Этот тип может быть клонирован и передан между потоками для создания нескольких точек отправки сообщений.
/// Cтоит иметь ввиду, что непосредственная отправка сообщений внутри коннектора реализована
/// с использованем блокирующих примитивов синхронизации, происходит в "последовательном" режиме
/// и не гарантирует очерёдность.
///
/// # Пример
/// ```no_run
/// use libtxc::{TransaqConnector, Sender};
///
/// let txc = /*..*/;
/// txc.subscribe(|buf| println!("rx: {}", buf.to_string_lossy()));
///
/// let sender: Sender = txc.sender();
/// let sender_2: Sender = sender.clone();
/// let cmd = "<command id=\"get_connector_version\"/>\0";
///
/// let h = std::thread::spawn(move ||{
///     unsafe{ sender.send(cmd) }.unwrap();
/// });
///
/// unsafe{ sender_2.send(cmd) }.unwrap();
///
/// h.join().unwrap();
///
/// // > rx: <connector_version>*.*.*.*</connector_version>
/// // > rx: <connector_version>*.*.*.*</connector_version>
/// ```
// `txc` library, by design, have a chance for a 'deadlock' on misuse, which can be prevented
// with the `rust` type system.
// In particular, the library is meant to be used in multi-threaded environment, access to the internal
// state is synchronized with the 'mutex' and user-callback is running inside the locked section.
// Thus, calling library functions from within the 'callback' implies taking a lock on the 'mutex'
// that is already locked by the 'callback' execution routine.
// User 'callback' required to be `Send` + `Sync` since it's executing on the different thread
// managed by the library, and to prevent `Sender` from moving into the 'callback' it must not
// meet one of these bounds, that is what this `*mut ` for
#[derive(Clone)]
pub struct Sender(Arc<Inner>, std::marker::PhantomData<*mut ()>);
unsafe impl Send for Sender {}

impl Sender {
    fn new(inner: Arc<Inner>) -> Self {
        Self(inner, std::marker::PhantomData)
    }

    /// Передаёт данные коннектору
    ///
    /// Передаёт буфер в функцию коннектора `BYTE* send_command(BYTE*)` и возвращает
    /// [`Result`] с ответным сообщением.
    ///
    /// Этот метод реализован через вызов [`Sender::send_ptr()`] с указателем на переданный буфер.
    ///
    /// # Safety
    /// Требования к буферу:
    /// - должен содержать только символы в кодировке UTF-8
    /// - содержать нулевой байт `\0`
    /// - оставаться валидным и не изменяться до окончания вызова метода
    ///
    /// # Errors
    /// - [`Error::InvalidCommand`] - при формировании команды была допущена ошибка и она не прошла
    /// проверку, или нарушена логика работы с коннектором
    /// - [`Error::Internal`] - во время обработки команды произошло исключение
    ///
    /// # Examples
    /// ```no_run
    /// let sender = /**/;
    ///
    /// let result = unsafe{ sender.send(r#"invalid command\0"#) };
    /// println!("{:?}", result);
    /// // > Err(Internal("<error>Error document empty.</error>"))
    /// ```
    /// ```no_run
    /// let result = unsafe { sender.send(r#"<command id="server_status"/>\0"#)};
    /// println!("{:?}", result);
    /// // > Err(InvalidCommand("<result success=\"false\"><message>Cannot process this command without connection.</message></result>"))
    /// ```
    /// ```no_run
    /// let result = unsafe{ sender.send(r#"<command id="get_connector_version"/>\0"#) };
    /// assert!(result.is_ok());
    /// println!("{}", result.unwrap().to_string_lossy());
    /// // > <result success="true"/>
    /// // > callback > <connector_version>*.*.*.*.*</connector_version>
    /// ```
    ///
    /// # Panics
    /// В `debug` сборке - если буфер не соответствует условиям
    #[inline]
    pub unsafe fn send<B: AsRef<[u8]>>(&self, buf: B) -> Result<TCStr<'_>> {
        #[cfg(debug_assertions)]
        {
            let non_empty = !buf.as_ref().is_empty();
            let contains_term = buf.as_ref().iter().any(|b| b'\0'.eq(b));
            let valid_utf8 = std::str::from_utf8(buf.as_ref()).is_ok();
            assert!(non_empty, "пустой буфер");
            assert!(contains_term, "отсутствует нулевой байт");
            assert!(valid_utf8, "буфер содержит не валидные UTF-8 символы");
        }

        self.send_ptr(buf.as_ref().as_ptr())
    }

    /// Передаёт данные коннектору
    ///
    /// Передаёт указатель на данные в функцию коннектора `BYTE* send_command(BYTE*)` и возвращает
    /// [`Result`] с ответным сообщением.
    ///
    /// Этот метод не имеет накладных расходов, но его использование может привести к **undefined behaviour**
    /// в случае нарушения любого из условий, поэтому он определён как небезопасный(*unsafe*).
    ///
    /// Если опция проекта(feature) **safe_buffers** не включена, то парсинг результата сводится
    /// к чтению 1-2х байт по фиксированным смещениям. При включенной опции - определяется размер
    /// буфера(`libc::strlen`) и проверяется выход за границы.
    ///
    /// # Safety
    /// Помимо требований к указателям(см. [std::ptr] safety invariants) языка `rust`,
    /// память, на которую ссылается указатель:
    /// - должна содержать только символы в кодировке UTF-8
    /// - содержать нулевой байт `\0`
    /// - оставаться валидной и не изменяться до окончания вызова метода
    ///
    /// # Errors
    /// - [`Error::InvalidCommand`] - при формировании команды была допущена ошибка и она не прошла
    /// проверку, или нарушена логика работы с коннектором
    /// - [`Error::Internal`] - во время обработки команды произошло исключение
    ///
    /// # Panics
    /// В `debug` сборке - если передан нулевой указатель
    #[cfg_attr(feature = "tracing", instrument(level = "debug", skip_all))]
    #[inline]
    pub unsafe fn send_ptr(&self, ptr: *const u8) -> Result<TCStr<'_>> {
        debug_assert!(!ptr.is_null(), "нулевой указатель");

        as_nonnull_txc_buf(self.0.module.send_command(ptr) as _)
            .map(|ptr| TCStr::new(ptr, self.0.module.free_memory))
            .and_then(parse_send_response)
    }
}

/// Глубина логирования в соответствии с детализацией и размером лог-файла
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(i32)]
pub enum LogLevel {
    /// минимальный
    Minimum = 1,
    /// стандартный(по-умолчанию)
    #[default]
    Default = 2,
    /// максимальный
    Maximum = 3,
}
impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            LogLevel::Minimum => "LogLevel::Minimum",
            LogLevel::Default => "LogLevel::Default",
            LogLevel::Maximum => "LogLevel::Maximum",
        })
    }
}
impl From<i32> for LogLevel {
    fn from(value: i32) -> Self {
        unsafe { std::mem::transmute(value.clamp(1, 3)) }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Loading(inner) => {
                write!(f, "Не удалось загрузить библиотеку: {inner} ")
            }
            Error::Initialization(msg) => {
                write!(f, "Инициализация библиотеки трагически провалилась: {msg} ")
            }
            Error::InvalidCommand(msg) => {
                write!(f, "Команда не прошла проверку и не была отправлена: {msg} ")
            }
            Error::Internal(msg) => {
                write!(
                    f,
                    "Внутренняя ошибка/exception коннектора - скорее всего это - исключительная ситуация, \
                       которая не должна происходить в принципе.\n\
                       Проверьте целостность файлов, актуальность версий и параметры окружения.\n\
                       Если ситуация повторяется и надежда иссякла, вы могли бы создать issue на\
                       github.\n{msg}"
                )
            }
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        if let Self::Loading(src) = self {
            Some(src)
        } else {
            None
        }
    }
}

unsafe impl Send for Error {}
unsafe impl Sync for Error {}

impl fmt::Debug for TransaqConnector {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("TransaqConnector").finish()
    }
}

impl fmt::Debug for Sender {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Sender").finish()
    }
}

//
// (C) hashbrown
//
// Branch prediction hint. This is currently only available on nightly but it
// consistently improves performance by 10-15%.
#[cfg(feature = "nightly")]
use core::intrinsics::{likely, unlikely};

// On stable we can use #[cold] to get a equivalent effect: this attributes
// suggests that the function is unlikely to be called
#[cfg(not(feature = "nightly"))]
#[inline]
#[cold]
fn cold() {}

#[allow(unused)]
#[cfg(not(feature = "nightly"))]
#[inline]
fn likely(b: bool) -> bool {
    if !b {
        cold();
    }
    b
}

#[allow(unused)]
#[cfg(not(feature = "nightly"))]
#[inline]
fn unlikely(b: bool) -> bool {
    if b {
        cold();
    }
    b
}

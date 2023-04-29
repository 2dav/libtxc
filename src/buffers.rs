use super::{ffi, Error};
use std::{ffi::CStr, fmt, ops::Deref, ptr::NonNull};
#[cfg(feature = "tracing")]
use tracing::instrument;

#[inline(always)]
pub fn as_nonnull_txc_buf(p: *mut u8) -> Result<NonNull<u8>, Error> {
    // though this condition is part of the `safe_buffers` feature contract,
    // it will be successfully predicted all the time and is good to have always on
    if super::likely(!p.is_null()) {
        Ok(unsafe { NonNull::new_unchecked(p) })
    } else {
        Err(Error::Internal("Коннектор вернул нулевой указатель".into()))
    }
}

/// Указатель на буфер коннектора
///
/// Передаётся в пользовательскую функцию обратного вызова и возвращается в качестве результата
/// отправки команды.
///
/// Ассоциированный буфер освобождается автоматически в момент окончания времени жизни `TCStr`.
///
/// # Пример
/// ```no_run
/// use libtxc::TCStr;
///
/// let buf: TCStr = ...;
/// // Cпособы чтения содержимого реализованы через `deref` к `std::ffi::CStr`
/// let msg: std::borrow::Cow<str> = buf.to_string_lossy();
/// let msg: &[u8] = buff.as_bytes();
/// let msg: &str = unsafe{ std::str::from_raw_parts_unchecked(buf.as_ptr(), buf.len()) };
/// ```
pub struct TCStr<'a>(NonNull<u8>, ffi::FreeMemory, std::marker::PhantomData<&'a ()>);

impl TCStr<'_> {
    #[inline(always)]
    pub(crate) fn new(ptr: NonNull<u8>, free_mem: ffi::FreeMemory) -> Self {
        Self(ptr, free_mem, std::marker::PhantomData)
    }
}

impl Drop for TCStr<'_> {
    #[cfg_attr(feature = "tracing", instrument(level = "debug", skip_all))]
    #[inline]
    fn drop(&mut self) {
        let result = unsafe { (self.1)(self.as_ptr() as _) };
        if super::unlikely(!result) {
            eprintln!(
                "Операция очистки txc буфера FreeMemory(*) завершилась неудачно, \
                  это - недокументированная ситуация и возможно всякое. \
                  Cоздайте issue на github если вам удалось добиться воспроизводимости."
            );
        }
    }
}
impl Deref for TCStr<'_> {
    type Target = CStr;

    #[cfg_attr(feature = "tracing", instrument(level = "debug", skip_all))]
    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe { CStr::from_ptr(self.0.as_ptr() as _) }
    }
}
impl fmt::Debug for TCStr<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("TCStr").field(&self.0).finish()
    }
}
impl fmt::Display for TCStr<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_string_lossy())
    }
}

/* Response might be of three forms:
 * Success:   <result success=”true” ... />
 * Error:     <result success=”false”>...</result>
 * Exception: <error>...</error> */
#[allow(unused)]
const MIN_RESPONSE_LENGTH: usize = 15;
const MIN_RESULT_LENGTH: usize = 23;
const DEFINING_BYTE: usize = 1;
const RESULT_BOOL_START: usize = 17;

#[inline]
fn is_result(bytes: &[u8]) -> bool {
    b'r'.eq(unsafe { bytes.get_unchecked(DEFINING_BYTE) })
}
#[inline]
fn is_success(bytes: &[u8]) -> bool {
    b't'.eq(unsafe { bytes.get_unchecked(RESULT_BOOL_START) })
}

#[cfg(feature = "safe_buffers")]
#[inline(always)]
pub fn parse_send_response(buf: TCStr) -> super::Result<TCStr> {
    let bytes = buf.to_bytes();
    let len = bytes.len();

    if len < MIN_RESPONSE_LENGTH || (is_result(bytes) && len < MIN_RESULT_LENGTH) {
        return Err(Error::Internal(format!(
            "Коннектор вернул неожиданное сообщение \"{}\"",
            buf.to_string_lossy()
        )));
    }

    if super::likely(is_result(bytes) && is_success(bytes)) {
        Ok(buf)
    } else {
        let msg = buf.to_string_lossy().to_string();
        Err(if is_result(bytes) { Error::InvalidCommand(msg) } else { Error::Internal(msg) })
    }
}

#[cfg(not(feature = "safe_buffers"))]
#[inline(always)]
pub fn parse_send_response(buf: TCStr) -> super::Result<TCStr> {
    // this version skips implied `strlen`, bounds checks and UTF-8 validation
    unsafe {
        let bytes = std::slice::from_raw_parts(buf.as_ptr() as *const u8, MIN_RESULT_LENGTH);

        if super::likely(is_result(bytes) && is_success(bytes)) {
            Ok(buf)
        } else {
            let bytes = buf.to_bytes();
            let msg = std::str::from_utf8_unchecked(bytes).to_string();
            Err(if is_result(bytes) { Error::InvalidCommand(msg) } else { Error::Internal(msg) })
        }
    }
}

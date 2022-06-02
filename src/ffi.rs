use std::os::{
    raw::{c_int, c_void},
    windows::prelude::OsStrExt,
};
use std::{
    ffi::{CStr, CString, OsStr},
    mem::transmute,
};
use winapi::{
    shared::minwindef::{FARPROC, HMODULE},
    um::{errhandlingapi as er, libloaderapi as ll},
};

type Initialize = unsafe extern "C" fn(*const u8, c_int) -> *const u8;
type SetLogLevel = unsafe extern "C" fn(c_int) -> *const u8;
type SendCommand = unsafe extern "C" fn(*const u8) -> *const u8;
type FreeMemory = unsafe extern "C" fn(*const u8) -> bool;
type UnInitialize = unsafe extern "C" fn() -> *const u8;
type CallbackEx = extern "C" fn(*const u8, *mut c_void) -> bool;
type Callback = extern "C" fn(*const u8) -> bool;
type SetCallbackEx = unsafe extern "C" fn(CallbackEx, *const c_void) -> bool;
type SetCallback = unsafe extern "C" fn(Callback) -> bool;

/// Holds the handle of the instance of the loaded library and it's functions pointers
/// Handle is owned by the `Lib`, dropping it unloads corresponding library and
/// frees it's resources.
pub struct Lib {
    handle: HMODULE,
    _initialize: Initialize,
    _set_log_level: SetLogLevel,
    _send_command: SendCommand,
    _set_callback_ex: SetCallbackEx,
    _set_callback: SetCallback,
    _free_memory: FreeMemory,
    _uninitialize: UnInitialize,
}

#[inline]
unsafe fn into_result<T>(p: *mut T) -> Result<*mut T, std::io::Error> {
    if p.is_null() {
        Err(std::io::Error::from_raw_os_error(er::GetLastError() as i32))
    } else {
        Ok(p)
    }
}

/// Load library and returns ready to use wrapper over library functions
pub unsafe fn load<P: AsRef<OsStr>>(path: P) -> Result<Lib, std::io::Error> {
    {
        let wide_filename: Vec<u16> = path.as_ref().encode_wide().chain(Some(0)).collect();
        let mut prev_mode = 0;

        er::SetThreadErrorMode(1, &mut prev_mode);
        let h = ll::LoadLibraryW(wide_filename.as_ptr());
        er::SetThreadErrorMode(prev_mode, std::ptr::null_mut());

        into_result(h)
    }
    .and_then(|handle| {
        unsafe fn paddr(h: HMODULE, f: &[u8]) -> Result<FARPROC, std::io::Error> {
            let s = ll::GetProcAddress(h, f.as_ptr().cast());
            into_result(s)
        }
        let initialize = paddr(handle, b"Initialize\0")?;
        let set_loglevel = paddr(handle, b"SetLogLevel\0")?;
        let send_command = paddr(handle, b"SendCommand\0")?;
        let set_cb_ex = paddr(handle, b"SetCallbackEx\0")?;
        let set_cb = paddr(handle, b"SetCallback\0")?;
        let free_mem = paddr(handle, b"FreeMemory\0")?;
        let uninitialize = paddr(handle, b"UnInitialize\0")?;

        Ok(Lib {
            handle,
            _initialize: transmute(initialize),
            _set_log_level: transmute(set_loglevel),
            _send_command: transmute(send_command),
            _set_callback_ex: transmute(set_cb_ex),
            _set_callback: transmute(set_cb),
            _free_memory: transmute(free_mem),
            _uninitialize: transmute(uninitialize),
        })
    })
}

type BoxedClosurePtr = *mut Box<dyn FnMut(*const u8)>;

#[no_mangle]
extern "C" fn txc_callback_ex(p: *const u8, ctx: *mut c_void) -> bool {
    unsafe { (*(ctx as BoxedClosurePtr))(p) };
    true
}

impl Lib {
    /// txc::Initialize
    #[inline]
    pub fn initialize(&self, path: &CStr, log_level: c_int) -> *const u8 {
        unsafe { (self._initialize)(path.as_ptr().cast(), log_level) }
    }

    /// txc::SetLogLevel
    #[inline]
    pub fn set_log_level(&self, log_level: c_int) -> *const u8 {
        unsafe { (self._set_log_level)(log_level) }
    }

    /// txc::SendCommand
    #[inline]
    pub fn send_bytes(&self, cmd: &[u8]) -> *const u8 {
        unsafe { (self._send_command)(cmd.as_ptr()) }
    }

    /// txc::FreeMemory
    #[inline]
    pub fn free_memory(&self, pbuff: *const u8) -> bool {
        unsafe { (self._free_memory)(pbuff) }
    }

    /// txc::UnInitialize
    #[inline]
    pub fn uninitialize(&self) -> *const u8 {
        unsafe { (self._uninitialize)() }
    }

    /// txc::SetCallback
    #[inline]
    #[allow(unused)]
    pub fn set_callback_(&self, cb: Callback) -> bool {
        unsafe { (self._set_callback)(cb) }
    }

    /// txc::SetCallbackEx
    #[inline]
    pub fn set_callback_ex(&self, cb: CallbackEx, payload: *mut c_void) -> bool {
        unsafe { (self._set_callback_ex)(cb, payload) }
    }
}

impl Lib {
    /// Sets Rust closure as a c_void pointer callback by bridging it with additional indirection
    #[inline]
    pub fn set_callback<F>(&self, callback: F) -> bool
    where
        F: FnMut(*const u8),
    {
        let ctx: Box<Box<dyn FnMut(*const u8)>> = Box::new(Box::new(callback));
        self.set_callback_ex(txc_callback_ex, Box::into_raw(ctx).cast())
    }
}

impl Drop for Lib {
    fn drop(&mut self) {
        unsafe { ll::FreeLibrary(self.handle) };
    }
}

// Converts rust string to c-string(i.e. memcopy + null terminator)
#[inline]
pub(crate) fn to_cstring<S: AsRef<str>>(rstr: S) -> CString {
    CString::new(rstr.as_ref().as_bytes()).unwrap()
}

use std::os::{
    raw::{c_int, c_void},
    windows::prelude::OsStrExt,
};
use std::{
    ffi::{CStr, CString, OsStr},
    mem::transmute,
    result::Result,
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
type SetCallbackEx = unsafe extern "C" fn(CallbackEx, *const c_void) -> bool;

pub(crate) struct Lib {
    pub(crate) handle: HMODULE,
    _initialize: Initialize,
    _set_log_level: SetLogLevel,
    _send_command: SendCommand,
    _set_callback_ex: SetCallbackEx,
    _free_memory: FreeMemory,
    _uninitialize: UnInitialize,
}

pub(crate) unsafe fn load<P: AsRef<OsStr>>(path: P) -> Result<Lib, std::io::Error> {
    let wide_filename: Vec<u16> = path.as_ref().encode_wide().chain(Some(0)).collect();
    let mut prev_mode = 0;
    er::SetThreadErrorMode(1, &mut prev_mode);
    let handle = {
        let h = ll::LoadLibraryW(wide_filename.as_ptr());
        if h.is_null() {
            Err(std::io::Error::from_raw_os_error(er::GetLastError() as i32))
        } else {
            Ok(h)
        }
    };
    er::SetThreadErrorMode(prev_mode, std::ptr::null_mut());
    drop(wide_filename);
    handle.and_then(|h| {
        unsafe fn paddr(h: HMODULE, f: &[u8]) -> Result<FARPROC, std::io::Error> {
            let s = ll::GetProcAddress(h, f.as_ptr().cast());
            if s.is_null() {
                Err(std::io::Error::from_raw_os_error(er::GetLastError() as i32))
            } else {
                Ok(s)
            }
        }
        let initialize = paddr(h, b"Initialize\0")?;
        let set_loglevel = paddr(h, b"SetLogLevel\0")?;
        let send_command = paddr(h, b"SendCommand\0")?;
        let set_cb_ex = paddr(h, b"SetCallbackEx\0")?;
        let free_mem = paddr(h, b"FreeMemory\0")?;
        let uninitialize = paddr(h, b"UnInitialize\0")?;

        Ok(Lib {
            handle: h,
            _initialize: transmute(initialize),
            _set_log_level: transmute(set_loglevel),
            _send_command: transmute(send_command),
            _set_callback_ex: transmute(set_cb_ex),
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
    #[inline]
    pub fn initialize(&self, path: &CStr, log_level: c_int) -> *const u8 {
        unsafe { (self._initialize)(path.as_ptr().cast(), log_level) }
    }

    #[inline]
    pub fn set_log_level(&self, log_level: c_int) -> *const u8 {
        unsafe { (self._set_log_level)(log_level) }
    }

    #[inline]
    pub fn send_bytes(&self, cmd: &[u8]) -> *const u8 {
        unsafe { (self._send_command)(cmd.as_ptr()) }
    }

    #[inline]
    pub fn free_memory(&self, pbuff: *const u8) -> bool {
        unsafe { (self._free_memory)(pbuff) }
    }

    #[inline]
    pub fn uninitialize(&self) -> *const u8 {
        unsafe { (self._uninitialize)() }
    }

    #[inline]
    pub fn set_callback<F, R>(&self, callback: F) -> bool
    where
        F: FnMut(*const u8) -> R,
    {
        // double indirection needed to get a c_void compatible pointer from trait object pointer
        let ctx: Box<Box<dyn FnMut(*const u8) -> R>> = Box::new(Box::new(callback));
        unsafe { (self._set_callback_ex)(txc_callback_ex, Box::into_raw(ctx) as *mut c_void) }
    }
}

impl Drop for Lib {
    fn drop(&mut self) {
        unsafe { ll::FreeLibrary(self.handle) };
    }
}

// Converts rust string to c-string(i.e. memcopy + null terminator)
#[inline(always)]
pub(crate) fn to_cstring<S: AsRef<str>>(rstr: S) -> CString {
    CString::new(rstr.as_ref().as_bytes()).unwrap()
}

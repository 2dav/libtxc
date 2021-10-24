use std::ffi::{CStr, CString, OsStr};
use std::mem::MaybeUninit;
use std::os::windows::prelude::OsStrExt;
use std::ptr::addr_of_mut;
use std::result::Result;

use std::os::raw::{c_int, c_void};
use winapi::shared::minwindef::{FARPROC, HMODULE};
use winapi::um::{errhandlingapi as er, libloaderapi as ll};

pub(crate) struct Lib {
    handle: HMODULE,
    _initialize: FARPROC,
    _set_log_level: FARPROC,
    _send_command: FARPROC,
    _set_callback_ex: FARPROC,
    _free_memory: FARPROC,
    _uninitialize: FARPROC,
}

pub(crate) unsafe fn load<P: AsRef<OsStr>>(path: P) -> Result<Lib, std::io::Error> {
    let wide_filename: Vec<u16> = path.as_ref().encode_wide().chain(Some(0)).collect();
    let mut prev_mode = 0;
    er::SetThreadErrorMode(1, &mut prev_mode);
    let handle = {
        let h = ll::LoadLibraryExW(wide_filename.as_ptr(), std::ptr::null_mut(), 0);
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
        let mut uninit = MaybeUninit::<Lib>::uninit();
        let ptr = uninit.as_mut_ptr();

        let initialize = paddr(h, b"Initialize\0")?;
        addr_of_mut!((*ptr)._initialize).write(initialize);

        let set_loglevel = paddr(h, b"SetLogLevel\0")?;
        addr_of_mut!((*ptr)._set_log_level).write(set_loglevel);

        let send_command = paddr(h, b"SendCommand\0")?;
        addr_of_mut!((*ptr)._send_command).write(send_command);

        let set_cb_ex = paddr(h, b"SetCallbackEx\0")?;
        addr_of_mut!((*ptr)._set_callback_ex).write(set_cb_ex);

        let free_mem = paddr(h, b"FreeMemory\0")?;
        addr_of_mut!((*ptr)._free_memory).write(free_mem);

        let uninitialize = paddr(h, b"UnInitialize\0")?;
        addr_of_mut!((*ptr)._uninitialize).write(uninitialize);
        addr_of_mut!((*ptr).handle).write(h);
        Ok(uninit.assume_init())
    })
}

type Initialize = unsafe extern "C" fn(*const u8, c_int) -> *const u8;
type SetLogLevel = unsafe extern "C" fn(c_int) -> *const u8;
type SendCommand = unsafe extern "C" fn(*const u8) -> *const u8;
type FreeMemory = unsafe extern "C" fn(*const u8) -> bool;
type UnInitialize = unsafe extern "C" fn() -> *const u8;
type CallbackEx = extern "C" fn(*const u8, *mut c_void) -> bool;
type SetCallbackEx = unsafe extern "C" fn(CallbackEx, *const c_void) -> bool;

#[no_mangle]
extern "C" fn txc_callback_ex(p: *const u8, ctx: *mut c_void) -> bool {
    unsafe {
        let cb: &mut Box<dyn FnMut(*const u8)> = std::mem::transmute(ctx);
        cb(p);
    }
    true
}

impl Lib {
    #[inline(always)]
    unsafe fn pcast<T>(&self, p: &FARPROC) -> &T {
        &*(p as *const *mut _ as *const T)
    }

    #[inline(always)]
    pub fn initialize(&self, path: &CStr, log_level: c_int) -> *const u8 {
        unsafe { self.pcast::<Initialize>(&self._initialize)(path.as_ptr().cast(), log_level) }
    }

    #[inline(always)]
    pub fn set_log_level(&self, log_level: c_int) -> *const u8 {
        unsafe { self.pcast::<SetLogLevel>(&self._set_log_level)(log_level) }
    }

    #[inline(always)]
    pub fn send_command(&self, cmd: &CStr) -> *const u8 {
        unsafe { self.pcast::<SendCommand>(&self._send_command)(cmd.as_ptr().cast()) }
    }

    #[inline(always)]
    pub fn free_memory(&self, pbuff: *const u8) -> bool {
        unsafe { self.pcast::<FreeMemory>(&self._free_memory)(pbuff) }
    }

    #[inline(always)]
    pub fn uninitialize(&self) -> *const u8 {
        unsafe { self.pcast::<UnInitialize>(&self._uninitialize)() }
    }

    #[inline(always)]
    pub fn set_callback<F>(&self, callback: F) -> bool
    where
        F: FnMut(*const u8),
    {
        // double indirection needed to get a c_void compatible pointer from trait object pointer
        let ctx: Box<Box<dyn FnMut(*const u8)>> = Box::new(Box::new(callback));
        unsafe {
            self.pcast::<SetCallbackEx>(&self._set_callback_ex)(
                txc_callback_ex,
                Box::into_raw(ctx) as *mut c_void,
            )
        }
    }
}

impl Drop for Lib {
    fn drop(&mut self) {
        unsafe {
            ll::FreeLibrary(self.handle);
        }
    }
}

// Converts rust string to c-string(i.e. memcopy + null terminator)
#[inline(always)]
pub(crate) fn to_cstring<S: AsRef<str>>(rstr: S) -> CString {
    CString::new(rstr.as_ref().as_bytes()).unwrap()
}

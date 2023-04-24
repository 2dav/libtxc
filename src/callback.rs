use super::buffers::with_nonnull_buf;
use super::ffi::CallbackEx;
use super::stream::Stream;
use std::{ffi::c_void, mem, ptr::NonNull};

macro_rules! debug_assert_T_ptr {
    ($T:ty, $p:expr) => {
        debug_assert_eq!(false, $p.is_null());
        debug_assert_eq!(0, $p as usize & (mem::align_of::<$T>() - 1));
    };
}

macro_rules! eprintln_abort {
    ($x:tt) => {
        tracing::error!($x);
        eprintln!($x);
        std::process::abort()
    };
}

#[repr(transparent)]
pub struct InputStream<T>(pub T);

impl<T> Stream for InputStream<T>
where
    T: FnMut(CallbackEx, BoxT) + Send + Sync,
{
    type Output = NonNull<u8>;

    fn subscribe<F: FnMut(Self::Output) + Sync + Send + 'static>(mut self, f: F) {
        self.0(trampoline::<F>, BoxT::new(f))
    }
}

// 'trampoline' is registered as a 'callback' via `txc::set_callback_ex` and get's directly
// executed by the library within the C-language runtime.
//
// All these 'f::<F>' "trickery" are inlined at compile time, thus do not impose indirection to call 'F'.
// `callback: *mut c_void` is a pointer to 'F's state, if any.
extern "C" fn trampoline<F: FnMut(NonNull<u8>)>(buffer: *const u8, callback: *mut c_void) -> bool {
    #[cold]
    #[inline]
    fn null_ptr_error_abort() {
        eprintln_abort!("Коннектор вернул нулевой указатель");
    }

    let span = tracing::debug_span!("trampoline").entered();
    with_nonnull_buf(buffer as _, |ptr| run_callback::<F>(callback, ptr), null_ptr_error_abort);
    drop(span);

    true
}

#[cfg(not(feature = "catch_unwind"))]
#[inline(always)]
fn run_callback<F: FnMut(NonNull<u8>)>(callback: *mut c_void, buffer: NonNull<u8>) {
    unsafe { call_fn_ptr::<F>(callback, buffer) };
}

#[cfg(feature = "catch_unwind")]
#[inline(always)]
fn run_callback<F: FnMut(NonNull<u8>)>(callback: *mut c_void, buffer: NonNull<u8>) {
    #[cold]
    #[inline]
    fn closure_panic_abort(err: Box<dyn std::any::Any + Send>) -> ! {
        let panic_info = err
            .downcast::<String>()
            .map(|v| *v)
            .or_else(|e| e.downcast::<&str>().map(|v| v.to_string()))
            .unwrap_or_else(|_| "Неизвестная причина".to_string());
        eprintln_abort!("Паника в ffi коде: {panic_info:?}\n");
    }

    if let Err(err) = std::panic::catch_unwind(|| {
        debug_assert_T_ptr!(F, callback);
        unsafe { call_fn_ptr::<F>(callback, buffer) };
    }) {
        closure_panic_abort(err)
    }
}

#[inline(always)]
unsafe fn call_fn_ptr<F: FnMut(NonNull<u8>)>(f: *mut c_void, arg: NonNull<u8>) {
    (*f.cast::<F>())(arg);
}

// `Box<T>` with types erased
#[derive(Debug)]
pub struct BoxT {
    boxed_ptr: *mut c_void,
    drop_fn: unsafe fn(*mut c_void),
}

unsafe impl Send for BoxT {}

impl Drop for BoxT {
    #[inline]
    fn drop(&mut self) {
        unsafe { (self.drop_fn)(self.boxed_ptr) };
    }
}

impl BoxT {
    #[inline]
    pub fn new<T>(f: T) -> Self {
        Self { boxed_ptr: Box::into_raw(Box::new(f)) as _, drop_fn: drop_t::<T> }
    }

    #[inline]
    pub fn as_raw_ptr(&self) -> *mut c_void {
        self.boxed_ptr
    }
}

unsafe fn drop_t<T>(ptr: *mut c_void) {
    debug_assert_T_ptr!(T, ptr);
    let _ = Box::from_raw(ptr.cast::<T>());
}

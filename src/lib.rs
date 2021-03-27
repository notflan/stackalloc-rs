//! # Safe runtime stack allocations
//!
//! Provides methods for Rust to access and use runtime stack allocated buffers in a safe way.
//! This is accomplished through a helper function that takes a closure of `FnOnce` that takes the stack allocated buffer slice as a parameter.
//! The slice is considered to be valid only until this closure returns, at which point the stack is reverted back to the caller of the helper function. If you need a buffer that can be moved, use `Vec` or statically sized arrays.
//! The memory is allocated on the closure's caller's stack frame, and is deallocated when the caller returns.
//!
//! This slice will be properly formed with regards to the expectations safe Rust has on slices.
//! However, it is still possible to cause a stack overflow by allocating too much memory, so use this sparingly and never allocate unchecked amounts of stack memory blindly.
//!
//! # Examples
//! Allocating a byte buffer on the stack.
//! ```
//! # use std::io::{self, Write, Read};
//! # use stackalloc::*;
//! fn copy_with_buffer<R: Read, W: Write>(mut from: R, mut to: W, bufsize: usize) -> io::Result<usize>
//! {
//!   alloca_zeroed(bufsize, move |buf| -> io::Result<usize> {
//!    let mut read;
//!    let mut completed = 0;
//!    while { read = from.read(&mut buf[..])?; read != 0} {
//!     to.write_all(&buf[..read])?;
//!     completed += read;
//!    }
//!    Ok(completed)
//!   })
//! }
//! ```
//! ## Arbitrary types
//! Allocating a slice of any type on the stack.
//! ```
//! # use stackalloc::stackalloc;
//! # fn _prevent_attempted_execution() {
//! stackalloc(5, "str", |slice: &mut [&str]| {
//!  assert_eq!(&slice[..], &["str"; 5]);
//! });
//! # }
//! ```
//! ## Dropping
//! The wrapper handles dropping of types that require it.
//! ```
//! # use stackalloc::stackalloc_with;
//! # fn _prevent_attempted_execution() {
//! stackalloc_with(5, || vec![String::from("string"); 10], |slice| {
//!  assert_eq!(&slice[0][0][..], "string");  
//! }); // The slice's elements will be dropped here
//! # }
//! ```
//! # `MaybeUninit`
//! You can get the aligned stack memory directly with no initialisation.
//! ```
//! # use stackalloc::stackalloc_uninit;
//! # use std::mem::MaybeUninit;
//! # fn _prevent_attempted_execution() {
//! stackalloc_uninit(5, |slice| {
//!  for s in slice.iter_mut()
//!  {
//!    *s = MaybeUninit::new(String::new());
//!  }
//!  // SAFETY: We have just initialised all elements of the slice.
//!  let slice = unsafe { stackalloc::helpers::slice_assume_init_mut(slice) };
//!
//!  assert_eq!(&slice[..], &vec![String::new(); 5][..]);
//!
//!  // SAFETY: We have to manually drop the slice in place to ensure its elements are dropped, as `stackalloc_uninit` does not attempt to drop the potentially uninitialised elements.
//!  unsafe {
//!    std::ptr::drop_in_place(slice as *mut [String]);
//!  }
//! });
//! # }
//! ```
//!
//! # Performance
//! For small (1k or lower) element arrays `stackalloc` can outperform `Vec` by about 50% or more. This performance difference decreases are the amount of memory allocated grows.
//!
//! * test tests::bench::stackalloc_of_uninit_bytes_known   ... bench:           3 ns/iter (+/- 0)
//! * test tests::bench::stackalloc_of_uninit_bytes_unknown ... bench:           3 ns/iter (+/- 0)
//! * test tests::bench::stackalloc_of_zeroed_bytes_known   ... bench:          22 ns/iter (+/- 0)
//! * test tests::bench::stackalloc_of_zeroed_bytes_unknown ... bench:          17 ns/iter (+/- 0)
//! * test tests::bench::vec_of_uninit_bytes_known          ... bench:          13 ns/iter (+/- 0)
//! * test tests::bench::vec_of_uninit_bytes_unknown        ... bench:          55 ns/iter (+/- 0)
//! * test tests::bench::vec_of_zeroed_bytes_known          ... bench:          36 ns/iter (+/- 2)
//! * test tests::bench::vec_of_zeroed_bytes_unknown        ... bench:          37 ns/iter (+/- 0)
//!
//! # License
//! MIT licensed

#![cfg_attr(nightly, feature(test))] 

#![allow(dead_code)]

#[cfg(nightly)] extern crate test;

use std::{
    mem::{
	self,
	MaybeUninit,
	ManuallyDrop,
    },
    panic::{
	self,
	AssertUnwindSafe,
    },
    slice,
    ffi::c_void,
    ptr,
};

//TODO: pub mod avec; pub use avec::AVec;
mod ffi;

/// Allocate a runtime length uninitialised byte buffer on the stack, call `callback` with this buffer, and then deallocate the buffer.
///
/// Call the closure with a stack allocated buffer of `MaybeUninit<u8>` on the caller's frame of `size`. The memory is popped off the stack regardless of how the function returns (unless it doesn't return at all.)
///
/// # Notes
/// The buffer is allocated on the closure's caller's frame, and removed from the stack immediately after the closure returns (including a panic, or even a `longjmp()`).
///
/// # Panics
/// If the closure panics, the panic is propagated after cleanup of the FFI call stack.
///
/// # Safety
/// While this function *is* safe to call from safe Rust, allocating arbitrary stack memory has drawbacks.
///
/// ## Stack overflow potential
/// It is possible to cause a stack overflow if the buffer you allocate is too large. (This is possible in many ways in safe Rust.)
/// To avoid this possibility, generally only use this for small to medium size buffers of only runtime-known sizes (in the case of compile-time known sizes, use arrays. For large buffers, use `Vec`). The stack size can vary and what a safe size to `alloca` is can change throughout the runtime of the program and depending on the depth of function calls, but it is usually safe to do this.
/// However, **do not** pass unvalidated input sizes (e.g. read from a socket or file) to this function, that is a sure way to crash your program.
///
/// This is not undefined behaviour however, it is just a kind of OOM and will terminate execution of the program.
///
/// ## 0 sizes
/// If a size of 0 is passed, then a non-null, non-aliased, and properly aligned dangling pointer on the stack is used to construct the slice. This is safe and there is no performance difference (other than no allocation being performed.)
///
/// ## Initialisation
/// The stack buffer is not explicitly initialised, so the slice's elements are wrapped in `MaybeUninit`. The contents of uninitialised stack allocated memory is *usually* 0.
///
/// ## Cleanup
/// Immediately after the closure exits, the stack pointer is reset, effectively freeing the buffer. The pointer used for the creation of the slice is invalidated as soon as the closure exits. But in the absense of `unsafe` inside the closure, it isn't possible to keep this pointer around after the frame is destroyed.
///
/// ## Panics
/// The closure can panic and it will be caught and propagated after exiting the FFI boundary and resetting the stack pointer.
///
/// # Internals
/// This function creates a shim stack frame (by way of a small FFI function) and uses the same mechanism as a C VLA to extend the stack pointer by the size provided (plus alignment). Then, this pointer is passed to the provided closure, and after the closure returns to the shim stack frame, the stack pointer is reset to the base of the caller of this function.
///
/// ## Inlining
/// In the absense of inlining LTO (which *is* enabled if possible), this funcion is entirely safe to inline without leaking the `alloca`'d memory into the caller's frame; however, the FFI wrapper call is prevented from doing so in case the FFI call gets inlined into this function call.
/// It is unlikely the trampoline to the `callback` closure itself can be inlined.
pub fn alloca<T, F>(size: usize, callback: F) -> T
where F: FnOnce(&mut [MaybeUninit<u8>]) -> T
{
    let mut callback = ManuallyDrop::new(callback);
    let mut rval = MaybeUninit::uninit();

    let mut callback = |allocad_ptr: *mut c_void| {
	unsafe {
	    let slice = slice::from_raw_parts_mut(allocad_ptr as *mut MaybeUninit<u8>, size);
	    let callback = ManuallyDrop::take(&mut callback);
	    rval = MaybeUninit::new(panic::catch_unwind(AssertUnwindSafe(move || callback(slice))));
	}
    };

    /// Create and use the trampoline for input closure `F`.
    #[inline(always)] fn create_trampoline<F>(_: &F) -> ffi::CallbackRaw
    where F: FnMut(*mut c_void)
    {
	unsafe extern "C" fn trampoline<F: FnMut(*mut c_void)>(ptr: *mut c_void, data: *mut c_void)
	{
	    (&mut *(data as *mut F))(ptr);
	}

	trampoline::<F>
    }

    let rval = unsafe {
	ffi::alloca_trampoline(size, create_trampoline(&callback), &mut callback as *mut _ as *mut c_void);
	rval.assume_init()
    };
    
    match rval
    {
	Ok(v) => v,
	Err(pan) => panic::resume_unwind(pan),
    }
}

/// A module of helper functions for slice memory manipulation
///
/// These are mostly re-implementations of unstable corelib functions in stable Rust.
pub mod helpers {
    use super::*;
    #[inline(always)] pub(crate) fn align_buffer_to<T>(ptr: *mut u8) -> *mut T
    {
	use std::mem::align_of;
	((ptr as usize) + align_of::<T>() - (ptr as usize) % align_of::<T>()) as *mut T
    }

    /// Convert a slice of `MaybeUninit<T>` to `T`.
    ///
    /// This is the same as the unstable core library function `MaybeUninit::slice_assume_init()`
    ///
    /// # Safety
    /// The caller must ensure all elements of `buf` have been initialised before calling this function.
    #[inline(always)] pub unsafe fn slice_assume_init<T>(buf: & [MaybeUninit<T>]) -> &[T]
    {
	& *(buf as *const [MaybeUninit<T>] as *const [T]) // MaybeUninit::slice_assume_init()
    }

    /// Convert a mutable slice of `MaybeUninit<T>` to `T`.
    ///
    /// This is the same as the unstable core library function `MaybeUninit::slice_assume_init_mut()`
    ///
    /// # Safety
    /// The caller must ensure all elements of `buf` have been initialised before calling this function.
    #[inline(always)] pub unsafe fn slice_assume_init_mut<T>(buf: &mut [MaybeUninit<T>]) -> &mut [T]
    {
	&mut *(buf as *mut [MaybeUninit<T>] as *mut [T]) // MaybeUninit::slice_assume_init_mut()
    }
}

use helpers::*;

/// Allocate a runtime length zeroed byte buffer on the stack, call `callback` with this buffer, and then deallocate the buffer.
///
/// See `alloca()`.
#[inline] pub fn alloca_zeroed<T, F>(size: usize, callback: F) -> T
where F: FnOnce(&mut [u8]) -> T
{
    alloca(size, move |buf| {
	    // SAFETY: We zero-initialise the backing slice
	    callback(unsafe {
		ptr::write_bytes(buf.as_mut_ptr(), 0, buf.len()); // buf.fill(MaybeUninit::zeroed());
		slice_assume_init_mut(buf)
	    })
	})
}


/// Allocate a runtime length slice of uninitialised `T` on the stack, call `callback` with this buffer, and then deallocate the buffer.
///
/// The slice is aligned to type `T`.
///
/// See `alloca()`.
#[inline] pub fn stackalloc_uninit<T, U, F>(size: usize, callback: F) -> U
where F: FnOnce(&mut [MaybeUninit<T>]) -> U
{
    let size_bytes = (std::mem::size_of::<T>() * size) + std::mem::align_of::<T>();
    alloca(size_bytes, move |buf| {
	    let abuf = align_buffer_to::<MaybeUninit<T>>(buf.as_mut_ptr() as *mut u8);
	    debug_assert!(buf.as_ptr_range().contains(&(abuf as *const _ as *const MaybeUninit<u8>)));
	    unsafe {
		callback(slice::from_raw_parts_mut(abuf, size))
	    }
	})
}

/// Allocate a runtime length slice of `T` on the stack, fill it by calling `init_with`, call `callback` with this buffer, and then drop and deallocate the buffer.
///
/// The slice is aligned to type `T`.
///
/// See `alloca()`.
#[inline] pub fn stackalloc_with<T, U, F, I>(size: usize, mut init_with: I, callback: F) -> U
where F: FnOnce(&mut [T]) -> U,
I: FnMut() -> T
{
    stackalloc_uninit(size, move |buf| {
	    buf.fill_with(move || MaybeUninit::new(init_with()));
	    // SAFETY: We have initialised the buffer above
	    let buf = unsafe { slice_assume_init_mut(buf) };
	    let ret = callback(buf);
	    if mem::needs_drop::<T>()
	    {
		// SAFETY: We have initialised the buffer above
		unsafe {
		    ptr::drop_in_place(buf as *mut _);
		}
	    }
	    ret
	})
}

/// Allocate a runtime length slice of `T` on the stack, fill it by cloning `init`, call `callback` with this buffer, and then drop and deallocate the buffer.
///
/// The slice is aligned to type `T`.
///
/// See `alloca()`.
#[inline] pub fn stackalloc<T, U, F>(size: usize, init: T, callback: F) -> U
where F: FnOnce(&mut [T]) -> U,
T: Clone
{
    stackalloc_with(size, move || init.clone(), callback)
}


/// Allocate a runtime length slice of `T` on the stack, fill it by calling `T::default()`, call `callback` with this buffer, and then drop and deallocate the buffer.
///
/// The slice is aligned to type `T`.
///
/// See `alloca()`.
#[inline] pub fn stackalloc_with_default<T, U, F>(size: usize, callback: F) -> U
where F: FnOnce(&mut [T]) -> U,
T: Default
{
    stackalloc_with(size, T::default, callback)
}

#[cfg(test)]
mod tests;

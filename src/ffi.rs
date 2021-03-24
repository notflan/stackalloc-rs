use std::ffi::c_void;

pub type CallbackRaw = extern "C" fn (ptr: *mut c_void, data: *mut c_void)->();

extern "C" {
    fn _alloca_trampoline(size: usize, cb: Option<CallbackRaw>, data: *mut c_void);
}

/// Call the `_alloca_trampoline` C function.
///
/// # Safety requirements & guarantees
/// * `size` should be small enough to not overflow the stack. A size of 0 is allowed.
/// * `cb` **must** catch any unwinds.
/// * `data` can be `null`, it is passed as the 2nd argument to `cb` as-is.
/// * The first argument to `cb` is guaranteed to be a non-aliased, properly aligned, and non-null pointer with `size` read+writable memory. If `size` is 0, it may dangle.
/// * `cb` is guaranteed to be called unless allocating `size` bytes on the stack causes a stack overflow, in which case the program will terminate.
/// * The data pointed to by `ptr` is guaranteed to be popped from the stack once this function returns (even in the case of a `longjmp`, but `panic!()` within the callback is still undefined behaviour).
// Never inline this, in case LTO inlines the call to `_alloca_trampoline`, we always want this function to pop the alloca'd memory.
// (NOTE: Test to see if this can ever happen. If it can't, the change this to `inline(always)` or remove the `inline` attribute.)
#[inline(never)] pub unsafe fn alloca_trampoline(size: usize, cb: CallbackRaw, data: *mut c_void)
{
    _alloca_trampoline(size, Some(cb), data);
}

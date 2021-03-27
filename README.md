# Safe runtime stack allocations

Provides methods for Rust to access and use runtime stack allocated buffers in a safe way (like C VLAs or the `alloca()` function.)
This is accomplished through a helper function that takes a closure of `FnOnce` that takes the stack allocated buffer slice as a parameter.
The slice is considered to be valid only until this closure returns, at which point the stack is reverted back to the caller of the helper function. If you need a buffer that can be moved, use `Vec` or statically sized arrays.
The memory is allocated on the closure's caller's stack frame, and is deallocated when the caller returns.

This slice will be properly formed with regards to the expectations safe Rust has on slices.
However, it is still possible to cause a stack overflow by allocating too much memory, so use this sparingly and never allocate unchecked amounts of stack memory blindly.

# Requirements
The crate works on stable or nightly Rust, but a C99-compliant compiler is required to build.

# Examples
Allocating a byte buffer on the stack.
```rust
# use std::io::{self, Write, Read};
# use stackalloc::*;
fn copy_with_buffer<R: Read, W: Write>(mut from: R, mut to: W, bufsize: usize) -> io::Result<usize>
{
  alloca_zeroed(bufsize, move |buf| -> io::Result<usize> {
   let mut read;
   let mut completed = 0;
   while { read = from.read(&mut buf[..])?; read != 0} {
    to.write_all(&buf[..read])?;
    completed += read;
   }
   Ok(completed)
  })
}
```
## Arbitrary types
Allocating a slice of any type on the stack.
```rust
# use stackalloc::stackalloc;
# fn _prevent_attempted_execution() {
stackalloc(5, "str", |slice: &mut [&str]| {
 assert_eq!(&slice[..], &["str"; 5]);
});
# }
```
## Dropping
The wrapper handles dropping of types that require it.
```rust
# use stackalloc::stackalloc_with;
# fn _prevent_attempted_execution() {
stackalloc_with(5, || vec![String::from("string"); 10], |slice| {
 assert_eq!(&slice[0][0][..], "string");  
}); // The slice's elements will be dropped here
# }
```
# `MaybeUninit`
You can get the aligned stack memory directly with no initialisation.
```rust 
# use stackalloc::stackalloc_uninit;
# use std::mem::MaybeUninit;
# fn _prevent_attempted_execution() {
stackalloc_uninit(5, |slice| {
 for s in slice.iter_mut()
 {
   *s = MaybeUninit::new(String::new());
 }
 // SAFETY: We have just initialised all elements of the slice.
 let slice = unsafe { stackalloc::helpers::slice_assume_init_mut(slice) };

 assert_eq!(&slice[..], &vec![String::new(); 5][..]);

 // SAFETY: We have to manually drop the slice in place to ensure its elements are dropped, as `stackalloc_uninit` does not attempt to drop the potentially uninitialised elements.
 unsafe {
   std::ptr::drop_in_place(slice as *mut [String]);
 }
});
# }
```

# How does it work?
Since Rust has no way to manipulate the stack at runtime, we use FFI to call into a function which manipulates *its* frame to allocate the desired memory there. Then, this funcion calls into a callback with a pointer to this stack allocated memory. Once the callback returns, the FFI function handles resetting the stack pointer to that of its caller.

This is beneficial compared to allocating the memory directly on the frame of the caller in many ways:
* In all languages that support `alloca`, it is implemented as a compiler builtin. This is because the compiler must insert instructions to reset the stack pointer at any point that the function returns, it must also keep track of addresses of anything pushed onto the stack after the call to `alloca`. Doing this manually would inhibit the compiler's ability to manage the stack pointer, and prevent it from being sure of not only the size of the current stack frame, but the layout of it as well. Therefore, it is likely more efficient.
* Allocating directly on the caller's frame would mean the memory stays allocated until the function *returns*, *not* until it falls out of scope. This can be a subtle source of bugs, such as the risk of `alloca()`ing in a loop causing a stack overflow. Therefore, it is also safer and less of a footgun.
* Implementing `alloc` this way would require platform-specific inline asm. Which is not only an unstable feature of Rust, it is incredibly unsafe as the compiler might make assumptions based on the layout of the stack which would then become invalidated. There is currently no way to implement a *safe* `alloca` that behaves this way, as such an implementation would require the user of this API to be very aware of where and how she can and can't use it.

## Downsides
This comes at the cost of an extra function call that is likely not inlineable (even with LTO and other aggressive optimisations).
The tradeoff of a stack allocated buffer over a heap allocated one may not be worth it in general, but this extra indirection could potentially impact performance on a micro scale.
If performance is of utmost importance to you, then you should be benchmarking these things anyway. It could be a heap allocated `Vec` is more performant in your use-case, or a stack allocated slice might be.

# Performance
For small (1k or lower) element arrays `stackalloc` can outperform `Vec` by about 50% or more. This performance difference decreases are the amount of memory allocated grows.

```
test tests::bench::stackalloc_of_uninit_bytes_known   ... bench:           3 ns/iter (+/- 0)
test tests::bench::stackalloc_of_uninit_bytes_unknown ... bench:           3 ns/iter (+/- 0)
test tests::bench::stackalloc_of_zeroed_bytes_known   ... bench:          22 ns/iter (+/- 0)
test tests::bench::stackalloc_of_zeroed_bytes_unknown ... bench:          17 ns/iter (+/- 0)
test tests::bench::vec_of_uninit_bytes_known          ... bench:          13 ns/iter (+/- 0)
test tests::bench::vec_of_uninit_bytes_unknown        ... bench:          55 ns/iter (+/- 0)
test tests::bench::vec_of_zeroed_bytes_known          ... bench:          36 ns/iter (+/- 2)
test tests::bench::vec_of_zeroed_bytes_unknown        ... bench:          37 ns/iter (+/- 0)
```


# License
MIT licensed

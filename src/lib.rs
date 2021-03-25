#![allow(dead_code)]

use std::{
    mem::{
	MaybeUninit,
	ManuallyDrop,
    },
    panic::{
	self,
	AssertUnwindSafe,
    },
    slice,
    ffi::c_void,
};

mod ffi;

pub fn with_alloca<T, F>(size: usize, callback: F) -> T
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


#[cfg(test)]
mod tests {
    #[test]
    fn with_alloca()
    {
	use std::mem::MaybeUninit;
	
	const SIZE: usize = 128;
	let sum = super::with_alloca(SIZE, |buf| {

	    println!("Buffer size is {}", buf.len());
	    for (i, x) in (1..).zip(buf.iter_mut()) {
		*x = MaybeUninit::new(i as u8);
	    }
	    eprintln!("Buffer is now {:?}", unsafe { std::mem::transmute::<_, & &mut [u8]>(&buf) });

	    buf.iter().map(|x| unsafe { x.assume_init() } as u64).sum::<u64>()
	});

	assert_eq!(sum, (1..=SIZE).sum::<usize>() as u64); 
    }
    #[test]
    fn raw_trampoline()
    {
	use std::ffi::c_void;

	let size: usize = 100;
	let output = {
	    let mut size: usize = size;
	    extern "C" fn callback(ptr: *mut c_void, data: *mut c_void)
	    {
		let size = unsafe {&mut *(data as *mut usize)};
		let slice = unsafe {
		    std::ptr::write_bytes(ptr, 0, *size);
		    std::slice::from_raw_parts_mut(ptr as *mut u8, *size)
		};
		println!("From callback! Size is {}", slice.len());

		for (i, x) in (0..).zip(slice.iter_mut())
		{
		    *x = i as u8;
		}

		*size = slice.iter().map(|&x| x as usize).sum::<usize>();
	    }

	    unsafe {
		super::ffi::alloca_trampoline(size, callback, &mut size as *mut usize as *mut _);
	    }
	    size
	};

	assert_eq!(output, (0..size).sum::<usize>());
    }
}


mod ffi;


#[cfg(test)]
mod tests {
    #[test]
    fn raw_trampoline()
    {
	use std::ffi::c_void;

	let mut size: usize = 100;
	extern "C" fn callback(ptr: *mut c_void, data: *mut c_void)
	{
	    let size = unsafe {&mut *(data as *mut usize)};
	    println!("From callback! Size is {}", *size);

	    *size = 0;
	}

	unsafe {
	    super::ffi::alloca_trampoline(size, callback, &mut size as *mut usize as *mut _);
	}

	assert_eq!(size, 0);
    }
}


mod ffi;


#[cfg(test)]
mod tests {
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

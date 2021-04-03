//! Contains tests and benchmarks

#[test]
fn unknown_size_iter()
{
    let iter = 10..;
    let result: u64 = iter.clone().take(10).sum();

    assert_eq!(result, super::stackalloc_with_iter(10, iter, |buf| buf.iter().copied().sum::<u64>()));
}

fn unknown_size_iter_len() {
    assert_eq!(super::stackalloc_with_iter(1024, 0..100, |buf| buf.len()), 100);
}

#[test]
fn exact_size_iter()
{
    let iter = vec![
	1,
	2,
	13,
	24,
	100,
    ];
    let len = iter.len();
    let result: u64 = iter.iter().copied().sum();

    assert_eq!(super::stackalloc_from_iter_exact(iter, |buf| {
	assert_eq!(buf.len(), len);
	buf.iter().copied().sum::<u64>()
    }), result);
}

#[test]
#[should_panic]
fn unwinding_over_boundary()
{
    super::alloca(120, |_buf| panic!());
}
#[test]
fn with_alloca()
{
    use std::mem::MaybeUninit;
    
    const SIZE: usize = 128;
    let sum = super::alloca(SIZE, |buf| {

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

#[test] fn non_primitive_type()
{
    assert_eq!(super::stackalloc(10, String::from("Hello world"), |strings| {
	strings.iter().cloned().collect::<String>()
    }), std::iter::repeat(String::from("Hello world")).take(10).collect::<String>());
}

#[test] fn primitive_type()
{
    assert_eq!(super::stackalloc(10, 12.0, |floats| {
	floats.iter().copied().map(|x| x / 2.0).sum::<f64>()
    }), std::iter::repeat(12.0).take(10).map(|x| x / 2.0).sum());
}

#[cfg(nightly)]
mod bench
{
    const SIZE: usize = 1024;
    use test::{black_box, Bencher};
    use std::mem::MaybeUninit;
    use lazy_static::lazy_static;

    lazy_static! {
	static ref SIZE_RANDOM: usize = {
	    use std::time;

	    let base = time::SystemTime::now().duration_since(time::UNIX_EPOCH).unwrap().as_millis() as u64;

	    ((base & 300) + 1024) as usize
	};
    }

    #[bench]
    fn vec_of_uninit_bytes_unknown(b: &mut Bencher)
    {
	let size = *SIZE_RANDOM;
	b.iter(|| {
	    black_box(vec![MaybeUninit::<u8>::uninit(); size]);
	})
    }
    #[bench]
    fn stackalloc_of_uninit_bytes_unknown(b: &mut Bencher)
    {
	let size = *SIZE_RANDOM;

	b.iter(|| {
	    black_box(crate::alloca(size, |b| {black_box(b);}));
	})
    }
    
    #[bench]
    fn stackalloc_of_zeroed_bytes_unknown(b: &mut Bencher)
    {
	let size = *SIZE_RANDOM;

	b.iter(|| {
	    black_box(crate::alloca_zeroed(size, |b| {black_box(b);}));
	})
    }
    
    #[bench]
    fn vec_of_zeroed_bytes_unknown(b: &mut Bencher)
    {
	let size = *SIZE_RANDOM;

	b.iter(|| {
	    black_box(vec![0u8; size]);
	})
    }
    #[bench]
    fn vec_of_zeroed_bytes_known(b: &mut Bencher)
    {
	b.iter(|| {
	    black_box(vec![0u8; SIZE]);
	})
    }
    #[bench]
    fn vec_of_uninit_bytes_known(b: &mut Bencher)
    {
	b.iter(|| {
	    black_box(vec![MaybeUninit::<u8>::uninit(); SIZE]);
	})
    }
    #[bench]
    fn stackalloc_of_uninit_bytes_known(b: &mut Bencher)
    {
	b.iter(|| {
	    black_box(crate::alloca(SIZE, |b| {black_box(b);}));
	})
    }

    #[bench]
    fn stackalloc_of_zeroed_bytes_known(b: &mut Bencher)
    {
	b.iter(|| {
	    black_box(crate::alloca_zeroed(SIZE, |b| {black_box(b);}));
	})
    }
}

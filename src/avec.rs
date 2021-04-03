//! A `Vec`-like wrapper type that only allocates if a provided buffer is first exhausted.
use std::mem::{
    MaybeUninit,
    ManuallyDrop,
};
use std::marker::{Send, Sync, PhantomData};
use std::ops::Drop;
use std::slice;

#[repr(C)]
#[derive(Debug)]
struct StackBuffer<T>
{
    fill_ptr: usize, 
    buf_ptr: *mut MaybeUninit<T>,
}
impl<T> Clone for StackBuffer<T>
{
    fn clone(&self) -> Self {
	Self{
	    fill_ptr: self.fill_ptr,
	    buf_ptr: self.buf_ptr,
	}
    }
}
impl<T> Copy for StackBuffer<T>{}

#[repr(C)]
#[derive(Debug, Clone)]
struct HeapBuffer<T>
{
    _fill_ptr: usize, // vec.len()
    buf: Vec<T>,
}

#[repr(C)]
union Internal<T>
{
    stack: StackBuffer<T>,
    heap: ManuallyDrop<HeapBuffer<T>>,
}

/// A growable vector with a backing slice that will move its elements to the heap if the slice space is exhausted.
pub struct AVec<'a, T>
{
    /// max size of `inner.stack` before it's moved to `inner.heap`.
    stack_sz: usize, 
    inner: Internal<T>,

    _stack: PhantomData<&'a mut [MaybeUninit<T>]>,
}
unsafe impl<'a, T> Send for AVec<'a, T>{}
unsafe impl<'a, T> Sync for AVec<'a, T>{}

impl<'a, T> Drop for AVec<'a, T>
{
    fn drop(&mut self) {
	if self.is_allocated() {
	    // All stack elements have been moved to the heap. Drop the heap buffer.
	    unsafe {
		ManuallyDrop::drop(&mut self.inner.heap);
	    }
	} else {
	    if std::mem::needs_drop::<T>() {
		// Drop the allocated stack elements in place
		unsafe {
		    std::ptr::drop_in_place(std::ptr::slice_from_raw_parts_mut(self.inner.stack.buf_ptr as *mut T, self.fill_ptr())); // I think this drops the elements, we don't need to loop.
		    /*
		    for x in slice::from_raw_parts_mut(self.inner.stack.buf_ptr, self.fill_ptr())
		    {
		    std::ptr::drop_in_place(x.as_mut_ptr());
		}*/
		}
	    }
	}
    }
}

impl<'a, T> AVec<'a, T>
{
    /// The current fill_ptr of this stack buffer
    fn fill_ptr(&self) -> usize
    {
	// SAFETY: Both fields are repr(C) with this element first
	unsafe {
	    self.inner.stack.fill_ptr
	}
    }

    /// Have the elements been moved to the heap?
    pub fn is_allocated(&self) -> bool
    {
	self.fill_ptr() >= self.stack_sz
    }
    
    /// Create a new `AVec` with this backing buffer.
    pub fn new(stack: &'a mut [MaybeUninit<T>]) -> Self
    {
	let (buf_ptr, stack_sz) = (stack.as_mut_ptr(), stack.len());

	Self {
	    stack_sz,
	    inner: Internal {
		stack: StackBuffer {
		    fill_ptr: 0,
		    buf_ptr,
		}
	    },
	    _stack: PhantomData
	}
    }

    fn move_to_heap(&mut self)
    {
	let buf: Vec<T> = unsafe {
	    slice::from_raw_parts(self.inner.stack.buf_ptr as *const MaybeUninit<T>, self.fill_ptr()).iter().map(|x| x.as_ptr().read()).collect()
	};
	self.inner = Internal {
	    heap: ManuallyDrop::new(HeapBuffer {
		_fill_ptr: self.stack_sz,
		buf,
	    }),
	};
    }
    
    /// Insert an element into this `AVec`.
    pub fn push(&mut self, item: T)
    {
	if self.is_allocated()
	{
	    unsafe {
		(*self.inner.heap).buf.push(item)
	    }
	} else {
	    unsafe {
		let ptr = self.inner.stack.fill_ptr;
		*self.inner.stack.buf_ptr.add(ptr) = MaybeUninit::new(item);
		self.inner.stack.fill_ptr += 1;

		if self.is_allocated() {
		    // Move all items to heap
		    self.move_to_heap();
		}
	    }
	}
    }

    /// The number of elements in this `AVec`.
    pub fn len(&self) -> usize
    {
	if self.is_allocated()
	{
	    unsafe {
		self.inner.heap.buf.len()
	    }
	} else {
	    self.fill_ptr()
	}
    }
}

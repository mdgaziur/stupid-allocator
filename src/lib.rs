#![feature(allocator_api)]
#![feature(nonnull_slice_from_raw_parts)]

use parking_lot::Mutex;
use std::alloc::{AllocError, Allocator, GlobalAlloc, Layout};

use libc::{intptr_t, sbrk};
use std::mem;
use std::ptr::{null, NonNull};

/// A very stupid allocator.
/// Allocates data in the following format:
///
/// ```md
/// |----------|----------|
/// |  header  |  data    |
/// |----------|----------|
/// ```
///
/// Segfaults when you try to do a BIG allocation
pub struct StupidAllocator {
    allocator: Mutex<AllocatorImpl>,
}

impl StupidAllocator {
    pub const fn new() -> Self {
        Self {
            allocator: Mutex::new(AllocatorImpl::new()),
        }
    }
}

unsafe impl GlobalAlloc for StupidAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.size() == 0 {
            return 0 as _;
        }

        self.allocator.lock().allocate(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.allocator
            .lock()
            .deallocate(NonNull::new(ptr).unwrap(), layout);
    }
}

unsafe impl Allocator for StupidAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let ptr = unsafe { self.allocator.lock().allocate(layout) };
        if ptr.is_null() {
            return Err(AllocError {});
        }

        Ok(NonNull::slice_from_raw_parts(
            unsafe { NonNull::new_unchecked(ptr) },
            layout.size(),
        ))
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        self.allocator.lock().deallocate(ptr, layout)
    }
}

struct AllocatorImpl {
    head: Block,
    is_initialized: bool,
}

impl AllocatorImpl {
    /// Creates a new allocator
    ///
    /// `head` is temporary in this case. When first allocation is done,
    /// a new `head` will be allocated in the heap and then replace the previous one.
    const fn new() -> Self {
        Self {
            // head will be replaced by heap allocated head when initialized
            head: Block {
                data: null::<&u8>() as *mut u8,
                layout: Layout::new::<()>(),
                next: null::<&Block>() as *mut _,
                is_free: false,
            },
            is_initialized: false,
        }
    }

    unsafe fn initialize(&mut self) -> bool {
        let mut block = {
            let block = request_block(Layout::new::<()>());
            if block.is_null() {
                return false;
            }

            block.read()
        };
        block.is_free = false;
        self.head = block;
        self.is_initialized = true;

        true
    }

    unsafe fn allocate(&mut self, layout: Layout) -> *mut u8 {
        if !self.is_initialized && !self.initialize() {
            return null::<&u8>() as _;
        }

        let block = self.head.find_first_fit(layout);
        if block.is_null() {
            let new_block = request_block(layout);
            if new_block.is_null() {
                return 0 as _;
            }
            self.head.insert(new_block);
            return (*new_block).data;
        }

        (*block).data
    }

    unsafe fn deallocate(&mut self, ptr: NonNull<u8>, _: Layout) {
        let block = self.head.with_ptr(ptr.as_ptr());
        if block.is_null() {
            return;
        }

        (*block).is_free = true;

        if (*block).next.is_null() {
            // we don't care whether it fails or not, because this is a stupid allocator
            sbrk(-(allocation_size((*block).layout.size()) as intptr_t));
        }
    }
}

/// Appends the size of Block and returns it
fn allocation_size(size: usize) -> usize {
    align(size + align(mem::size_of::<Block>()))
}

fn align(size: usize) -> usize {
    (size + (mem::size_of::<intptr_t>() - 1)) & !(mem::size_of::<intptr_t>() - 1)
}

/// Requests a block from OS and returns it
fn request_block(layout: Layout) -> *mut Block {
    unsafe {
        let block = sbrk(0) as *mut Block;

        if block.is_null() {
            return null::<&Block>() as *mut _;
        }

        let retval = sbrk(allocation_size(align(layout.size())) as intptr_t);
        if retval.is_null() || retval as intptr_t == -1 {
            return null::<&Block>() as *mut _;
        }

        {
            let block_mut = block.as_mut().unwrap();
            block_mut.data = block.add(1) as *mut u8;
            block_mut.next = null::<&Block>() as _;
            block_mut.layout = layout;
            block_mut.is_free = false;
        }

        block
    }
}

#[derive(Debug)]
struct Block {
    layout: Layout,
    data: *mut u8,
    next: *mut Self,
    is_free: bool,
}

impl Block {
    fn find_first_fit(&mut self, layout: Layout) -> *mut Block {
        if layout.size() < self.layout.size() || !self.is_free {
            return if self.next.is_null() {
                null::<&Block>() as *mut _
            } else {
                // Safety:
                // We've already checked whether it's null or not
                // The allocator puts valid `Block` in `next` field
                unsafe { (*self.next).find_first_fit(layout) }
            };
        }

        self as *mut _
    }

    fn insert(&mut self, next: *mut Block) {
        if self.next.is_null() {
            self.next = next;
        } else {
            unsafe {
                (*self.next).insert(next);
            }
        }
    }

    fn with_ptr(&mut self, ptr: *mut u8) -> *mut Block {
        if self.data == ptr {
            self as *mut _
        } else if !self.next.is_null() {
            unsafe { (*self.next).with_ptr(ptr) }
        } else {
            null::<&Block>() as *mut _
        }
    }
}

unsafe impl Send for Block {}

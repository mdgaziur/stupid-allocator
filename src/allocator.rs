use nix::libc::{intptr_t, sbrk};

use parking_lot::Mutex;
use std::alloc::{AllocError, Allocator as AllocatorTrait, GlobalAlloc, Layout};


use std::ptr::{null_mut, NonNull};

pub struct Allocator {
    allocator_impl: Mutex<AllocatorImpl>,
}

impl Allocator {
    pub const fn new() -> Self {
        Self {
            allocator_impl: Mutex::new(AllocatorImpl::new()),
        }
    }
}

unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.allocator_impl.lock().allocate(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        self.allocator_impl.lock().deallocate(ptr);
    }
}

unsafe impl AllocatorTrait for Allocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let ptr = unsafe { self.allocator_impl.lock().allocate(layout) };
        if ptr.is_null() {
            Err(AllocError {})
        } else {
            Ok(
                unsafe {
                    NonNull::slice_from_raw_parts(NonNull::new_unchecked(ptr), layout.size())
                },
            )
        }
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        self.dealloc(ptr.as_ptr(), layout);
    }
}

struct AllocatorImpl {
    head: Block,
}

impl AllocatorImpl {
    const DUMMY_BLOCK: Block = Block {
        data: 0 as *mut u8,
        size: 0,
        next: None,
        free: false,
    };

    pub const fn new() -> Self {
        Self {
            head: Self::DUMMY_BLOCK,
        }
    }

    pub unsafe fn allocate(&mut self, layout: Layout) -> *mut u8 {
        assert_ne!(layout.size(), 0, "zero sized allocation is UB!");

        let last_block = match self.head.find_with_size(layout.size()) {
            Ok(block) => return block.data,
            Err(block) => block,
        };

        let extended_layout = layout
            .extend(Layout::for_value(&Self::DUMMY_BLOCK))
            .unwrap()
            .0
            .pad_to_align();

        let block_addr = sbrk(
            (extended_layout.size() + extended_layout.padding_needed_for(layout.align()))
                as intptr_t,
        );
        if block_addr.is_null() || block_addr as intptr_t == -1 {
            return null_mut();
        }

        let block = &mut *(block_addr as *mut Block);
        block.data = block_addr.offset(1) as *mut u8;
        block.size = layout.size();
        block.free = false;
        block.next = None;
        last_block.next = Some(block_addr as *mut Block);

        block.data
    }
    pub fn deallocate(&mut self, ptr: *mut u8) {
        // TODO: do not leak memory
        self.head.find_by_ptr(ptr).map(|block| block.free = true);
    }
}

struct Block {
    data: *mut u8,
    size: usize,
    free: bool,
    next: Option<*mut Block>,
}

unsafe impl Send for Block {}

impl Block {
    fn find_with_size(&mut self, size: usize) -> Result<&mut Self, &mut Self> {
        if self.size == size && self.free {
            Ok(self)
        } else if let Some(next) = self.next {
            // Safety: The allocator ensures that the next block's
            //         address is valid.
            unsafe { next.as_mut() }.unwrap().find_with_size(size)
        } else {
            Err(self)
        }
    }

    fn find_by_ptr(&mut self, ptr: *mut u8) -> Option<&mut Self> {
        if self as *mut Block as *mut u8 == ptr {
            Some(self)
        } else if let Some(next) = self.next {
            unsafe { &mut *next }.find_by_ptr(ptr)
        } else {
            None
        }
    }
}

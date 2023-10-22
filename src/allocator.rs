use nix::libc::sbrk;

use spin::Mutex;
use std::alloc::{AllocError, Allocator as AllocatorTrait, GlobalAlloc, Layout};
use std::mem::{align_of, size_of};
use std::process::abort;

use std::ptr::{NonNull, null_mut};

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
        let alloca = self.allocator_impl.lock().allocate(layout);
        assert!(alloca.is_aligned());
        alloca
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        self.allocator_impl.lock().deallocate(ptr);
    }
}

unsafe impl AllocatorTrait for Allocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let ptr = self.allocator_impl.lock().allocate(layout);
        assert!(ptr.is_aligned());
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

unsafe impl Send for Block {}
unsafe impl Sync for Block {}

impl AllocatorImpl {
    const BLOCK0: Block = Block {
        data: NonNull::dangling().as_ptr(),
        size: 0,
        next: None,
        free: false,
    };

    pub const fn new() -> Self {
        Self { head: Self::BLOCK0 }
    }

    pub fn allocate(&mut self, layout: Layout) -> *mut u8 {
        let previous_break = unsafe { sbrk(0) } as usize;
        let block_addr = align_up(previous_break, align_of::<Block>());
        let alloc_sz = align_up(block_addr + size_of::<Block>(), layout.align()) + layout.size();

        let block = self
            .head
            .find_first_fit(alloc_sz);
        if let Some(block) = block {
            return block.data;
        }

        let new_brk = unsafe { sbrk((alloc_sz - previous_break) as isize) };
        if new_brk.is_null() {
            return null_mut();
        }

        let new_block_addr = align_up(new_brk as usize, align_of::<Block>());
        let mut new_block = NonNull::new(new_block_addr as *mut Block).unwrap();
        let data = align_up(new_block_addr + size_of::<Block>(), layout.align()) as *mut u8;
        unsafe {
            new_block.as_mut().size = layout.size();
            new_block.as_mut().next = None;
            new_block.as_mut().data = data;
            new_block.as_mut().free = false;
        }
        self.head.insert(new_block);

        data
    }

    pub unsafe fn deallocate(&mut self, ptr: *mut u8) {
        let block = self.head.find_by_ptr(ptr);
        if let Some(block) = block {
            if block.free {
                eprintln!("double free: {:?}", ptr);
                abort();
            }
            block.free = true;

            // collect all consecutive free blocks
            if let Some(ref mut final_block) = block.next {
                let mut final_block = final_block.as_mut();
                loop {
                    match final_block.next {
                        Some(ref mut next) => {
                            block.next = next.as_ref().next;
                            if unsafe { next.as_ref() }.free {
                                final_block = next.as_mut();
                            } else {
                                break;
                            }
                        }
                        None => break,
                    }
                }

                if final_block != block {
                    block.size += (final_block.data as usize + final_block.size) - block.data as usize;
                }
            }
        }
    }

    pub fn dump_blocks(&self) {
        let mut current_block = &self.head;
        let mut i = 1;

        loop {
            println!("|-------- Block #{i} --------|");
            println!("|- data: {:?}", current_block.data);
            println!("|- size: {:?}", current_block.size);
            println!("|- free: {:?}", current_block.free);
            println!("|- next: {:?}\n", current_block.next);
            match current_block.next {
                Some(ref next) => current_block = unsafe { next.as_ref() },
                None => break,
            }
            i += 1;
        }
    }
}

#[derive(PartialOrd, PartialEq)]
struct Block {
    data: *mut u8,
    size: usize,
    next: Option<NonNull<Block>>,
    free: bool,
}

impl Block {
    fn find_first_fit(&mut self, size: usize) -> Option<&Block> {
        if self.size >= size && self.free {
            Some(self)
        } else {
            match self.next {
                // SAFETY: block.next is a valid pointer to an instance of Block.
                Some(ref mut next) => unsafe { next.as_mut() }.find_first_fit(size),
                None => None,
            }
        }
    }

    fn find_by_ptr(&mut self, ptr: *mut u8) -> Option<&mut Block> {
        if self.data == ptr {
            Some(self)
        } else {
            match self.next {
                // SAFETY: block.next is a valid pointer to an instance of Block.
                Some(ref mut next) => unsafe { next.as_mut() }.find_by_ptr(ptr),
                None => None,
            }
        }
    }

    fn insert(&mut self, block: NonNull<Block>) {
        match self.next {
            // SAFETY: block.next is a valid pointer to an instance of Block.
            Some(ref mut next) => unsafe { next.as_mut() }.insert(block),
            None => self.next = Some(block),
        }
    }
}

fn align_up(addr: usize, align: usize) -> usize {
    assert!(align.is_power_of_two());
    (addr + align - 1) & !(align - 1)
}

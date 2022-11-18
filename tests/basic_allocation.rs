#![feature(allocator_api)]

use std::alloc::{Allocator, Layout};
use stupid_allocator::StupidAllocator;

#[test]
fn test_slice_allocation() {
    static ALLOCATOR: StupidAllocator = StupidAllocator::new();
    let mut ptr = ALLOCATOR.allocate(Layout::new::<[u8; 2]>()).unwrap();
    unsafe {
        ptr.as_mut()[0] = 69;
        ptr.as_mut()[1] = 255;

        assert_eq!(ptr.as_ref(), [69, 255])
    }
}

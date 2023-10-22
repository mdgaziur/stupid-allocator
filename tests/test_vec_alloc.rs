#![feature(allocator_api)]

use std::alloc::{GlobalAlloc, Layout};
use allocator_speedrun::allocator::Allocator;

#[global_allocator]
static ALLOCATOR: Allocator = Allocator::new();

#[test]
pub fn test_vec_alloc() {
    let allocator = Allocator::new();
    let mut v = Vec::with_capacity_in(1000000, &allocator);
    ALLOCATOR.dump_blocks();
    for i in 0..v.capacity() {
        v.push(i);
    }
    for i in 0..v.capacity() {
        assert_eq!(i, v[i]);
    }
}

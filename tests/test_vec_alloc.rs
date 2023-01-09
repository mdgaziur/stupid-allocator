#![feature(allocator_api)]

use allocator_speedrun::allocator::Allocator;

#[test]
pub fn test_vec_alloc() {
    let allocator = Allocator::new();
    let mut v = Vec::with_capacity_in(1000000, &allocator);
    for i in 0..v.capacity() {
        v.push(i);
    }
    for i in 0..v.capacity() {
        assert_eq!(i, v[i]);
    }
}
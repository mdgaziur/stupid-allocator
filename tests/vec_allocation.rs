#![feature(allocator_api)]

use stupid_allocator::StupidAllocator;

#[test]
fn test_vec_allocation() {
    static ALLOCATOR: StupidAllocator = StupidAllocator::new();
    let mut v: Vec<i32, _> = Vec::new_in(&ALLOCATOR);
    for i in 1..1000 {
        v.push(i);
    }
    for i in 1..1000 {
        assert_eq!(v[(i - 1) as usize], i);
    }
}
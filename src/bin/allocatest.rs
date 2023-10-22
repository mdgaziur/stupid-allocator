use allocator_speedrun::allocator::Allocator;

#[global_allocator]
static ALLOCATOR: Allocator = Allocator::new();

fn main() {
}
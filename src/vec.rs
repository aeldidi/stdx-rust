use core::alloc::Allocator;

struct Vec {
    alloc: dyn Allocator,
}

use crate::alloc;

struct Vec {
    alloc: dyn alloc::Allocator,
}

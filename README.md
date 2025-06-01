What?
-----

An alternative rust standard library. You can still use other things which make
use of `std`, I just don't always want to.

Why?
----

There are a couple main goals:

1. Be able to enforce that some code doesn't panic. Most APIs don't panic, and
   I'm going to implement a `#[nopanic]` macro to enforce that a function
   doesn't panic. It will give a compiler error if the function calls panic.

2. Implement a single-threaded event loop using `io_uring` and IO Completion
   Ports for IO. Everything possible should be async.

3. Expose everything as libraries. For example, I imagine the networking stack
   should have a `Connection` type,

The goal is to reduce the amount of code as much as possible, and using good
API design to help [shepherd][1] anyone making use of it to write faster and
more robust software.

Some concrete improvements I mean to make in compared to `std`:

- Custom allocators are explicit, arena allocation is reccomended
- Containers are as minimal as I can get them
- Possible to use while ensuring no panics happen

[1]: https://nibblestew.blogspot.com/2020/03/its-not-what-programming-languages-do.html

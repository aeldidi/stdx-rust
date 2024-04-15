What?
-----

An alternative rust standard library. You can still use other things which make
use of `std`, I just don't always want to.

Why?
----

The goal is to reduce the amount of code as much as possible, and using good
API design help [shepherd][1] anyone making use of it to write faster and more
robust software.

Some concrete improvements I mean to make in compared to `std`:

- Custom allocators are explicit, arena allocation is reccomended
- Containers are as minimal as I can get them
- Possible to use while ensuring no panics happen

[1]: https://nibblestew.blogspot.com/2020/03/its-not-what-programming-languages-do.html

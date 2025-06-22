
This module does a cool trick to allow using the parser both from within a
`proc_macro` and from within regular Rust code. Specifically, the `proc_macro`
types are availible both during compile time and runtime. If we declare all of
the `proc_macro` types as enums between the real types and a re-implementation
of the type, we can use the same code both in a proc macro and for regular
runtime parsing.

All the methods in both runtime and compile time implementations are marked as
`#[inline(always)]`, so whenever you use the actual exported type, only the
implementation you actually use should be compiled in.

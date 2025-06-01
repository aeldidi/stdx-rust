This crate only exists because of a weird quirk in the way Rust handles
dependencies. Essentially, procedural macros must be in their own crate, which
is fine, except for the fact that these proc macros also depend on some of the
`stdx` functionality.

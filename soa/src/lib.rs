extern crate proc_macro;

use proc_macro::TokenStream;
use stdx_core::rust;

#[proc_macro_derive(Soa)]
pub fn soa_derive(input: TokenStream) -> TokenStream {
    let decls = match rust::parse_type_decls(rust::TokenStream::from(input)) {
        Ok(x) => x,
        Err(e) => {
            return format!("::std::compile_error!(\"{}\")", e)
                .parse()
                .unwrap();
        }
    };

    if decls.len() > 1 {
        return format!(
            "{}{}",
            "::std::compile_error!(\"a derive macro can only be ",
            "used on a single type definition\")"
        )
        .parse()
        .unwrap();
    }

    let (name, fields) = match decls[0].clone() {
        rust::TypeDecl::Struct { name, fields } => (name, fields),
        _ => {
            return format!(
                "{}{}",
                r#"::std::compile_error!("derive(Soa) is only valid on "#,
                r#"structs with named fields")"#
            )
            .parse()
            .unwrap()
        }
    };

    let struct_name = format!("Soa{name}");
    // Implement the methods returning the slices for each field.
    let mut field_slice_methods = String::new();
    let mut offset_calculation = String::from("0");
    let mut single_element_size = String::from("0");
    let mut alignments = String::new();
    let mut field_types = Vec::<String>::new();
    let mut resize_body = String::new();
    let mut write_fields_from_value = String::new();
    let num_fields = fields.len();
    for field in &fields {
        let field_name = field.name.clone();
        let field_type_name = field.ty.clone();
        field_slice_methods.push_str(&format!(
            "
            #[inline]
            pub fn {field_name}s(&self) {{
                // SAFETY: the memory layout has each field's slice in
                //         contiguous memory, one after the other in the order
                //         they appear in the source code.
                unsafe {{ 
                    ::core::slice::from_raw_parts(
                        self.ptr.byte_offset({offset_calculation}).cast(),
                        self.len,
                    )
                }}
            }}

            #[inline]
            pub fn {field_name}s_mut(&mut self) {{
                // SAFETY: the memory layout has each field's slice in
                //         contiguous memory, one after the other in the order
                //         they appear in the source code.
                unsafe {{ 
                    ::core::slice::from_raw_parts(
                        self.ptr.byte_offset({offset_calculation}).cast(),
                        self.len,
                    )
                }}
            }}
            "
        ));

        // update the current offset calculation to go past this element.
        offset_calculation.push_str(&format!(
            " + ::core::mem::size_of<{field_type_name}>()*self.len"
        ));

        // add the field to the total size calculation.
        single_element_size.push_str(&format!(
            " + ::core::mem::size_of<{field_type_name}>()"
        ));

        // add the alignment expression to the list.
        alignments
            .push_str(&format!("::core::mem::align_of<{field_type_name}>(),"));

        // In the resize method, we move around all the arrays within the
        // capacity.
        resize_body.push_str(&format!(
            "
            old_cursor = align_up(
                old_cursor, ::core::mem::align_of<{field_type_name}>());
            cursor = align_up(
                cursor, ::core::mem::align_of<{field_type_name}>());
            ::core::ptr::copy::<u8>(
                self.ptr.byte_offset(old_cursor).cast(),
                new_ptr.byte_offset(cursor).cast(),
                self.len*::core::mem::size_of<{field_type_name}>());
            cursor += new_cap * ::core::mem::size_of<{field_type_name}>();
            new_cursor += self.cap * ::core::mem::size_of<{field_type_name}>();
            "
        ));

        // In all the push methods, we write a field from a variable called
        // value.
        write_fields_from_value.push_str(&format!(
            "
            cursor = align_up(
                cursor, ::core::mem::align_of<{field_type_name}>());
            ::core::ptr::write(
                self.ptr.byte_offset(cursor).cast(), value.{field_name});
            cursor += self.cap * ::core::mem::size_of<{field_type_name}>();
            "
        ));

        // add the type to our list of types.
        field_types.push(field_type_name);
    }

    format!(
        r#"
        struct {struct_name}<A: ::core::alloc::Allocator = ::alloc::alloc::Global>  {{
            /// The length of each array in elements.
            len: usize,
            /// The total number of elements we have allocated space for.
            cap: usize,
            /// The pointer to the allocated memory.
            ptr: ::core::ptr::NonNull<u8>,
            alloc: A,
        }}
        
        impl {struct_name} {{
            #[inline]
            pub fn new_in<A: ::core::alloc::Allocator>(alloc: A) -> Self<A> {{
                Self {{
                    len: 0,
                    cap: 0,
                    ptr: ::core::ptr::NonNull::<u8>::dangling(),
                    alloc: alloc,
                }}
            }}

            #[inline]
            pub fn new() -> Self<::alloc::alloc::Global> {{
                Self {{
                    len: 0,
                    cap: 0,
                    ptr: ::core::ptr::NonNull::<u8>::dangling(),
                    alloc: ::alloc::alloc::Global,
                }}
            }}

            #[inline]
            fn try_reserve_impl(&mut self, additional: usize) ->
                    ::core::result::Result<(), ::core::alloc::Layout> {{
                if self.len + additional < self.cap {{
                    return ::core::result::Result::Ok(());
                }}

                let new_cap = if self.cap == 0 {{ 16 }} else {{ self.cap*2 }}; 
                let layout = ::core::alloc::Layout::from_size_align(
                    new_cap*{single_element_size},
                    self.buf_align(),
                );
                if self.cap == 0 {{
                    if let Ok(ptr) = self.alloc.allocate(self.ptr, layout) {{
                        self.resize(new_cap, ptr);
                        self.ptr = ptr;
                        return;
                    }}

                    return ::core::result::Result::Err(layout);
                }}

                if let Ok(ptr) = self.alloc.grow(self.ptr, layout) {{
                    self.resize(new_cap, ptr.cast());
                    self.ptr = ptr.cast();
                    return;
                }}

                return ::core::result::Result::Err(layout);
            }}

            #[inline]
            pub fn try_reserve(&mut self, additional: usize) ->
                    ::core::result::Result<(), ::core::alloc::AllocError> {{
                match self.try_reserve_impl(additional) {{
                    ::core::result::Result::Err(..) =>
                        ::core::result::Result::Err(::core::alloc::AllocError),
                    _ => (),
                }}
            }}

            #[inline]
            pub fn reserve(&mut self, additional: usize) {{
                match self.try_reserve_impl(additional) {{
                    ::core::result::Result::Err(layout) =>
                        ::alloc::alloc::handle_alloc_error(layout),
                    _ => (),
                }}
            }}

            #[inline]
            pub fn try_push(&mut self, value: {name}) ->
                    ::core::result::Result<(), ::core::alloc::AllocError> {{
                const fn align_up(x: isize, a: isize) -> isize {{
                    debug_assert!(a.is_power_of_two());
                    (x + a - 1) & !(a - 1)
                }}
                self.try_reserve(self.len + 1)?;
                let mut cursor = 0;
                {write_fields_from_value}
                ::core::result::Result::Ok(())
            }}

            #[inline]
            pub fn push(&mut self, value: {name}) {{
                const fn align_up(x: isize, a: isize) -> isize {{
                    debug_assert!(a.is_power_of_two());
                    (x + a - 1) & !(a - 1)
                }}
                self.reserve(self.len + 1);

                let mut cursor = 0;
                {write_fields_from_value}
                ::core::result::Result::Ok(())
            }}

            #[inline]
            pub fn len() -> usize {{
                self.len
            }}

            #[inline]
            pub fn capacity() -> usize {{
                self.cap
            }}

            /// resizes the contents of `self.ptr` to be arranged in the new
            /// capacity given.
            /// 
            /// # Safety
            ///
            /// This function is `unsafe` because it performs raw pointer
            /// arithmetic and byte-wise moves.  
            /// 
            /// The **caller must uphold **all** of the following
            /// pre-conditions**:
            ///
            /// 1. `new_ptr` must be allocated using `self.alloc` to have
            ///    enough memory to fit `new_cap` elements of each array.
            /// 
            /// 2. `self.ptr` and `new_ptr` are aligned to the smallest
            ///    power-of-two which is greater than or equal to the maximum
            ///    alignment of each type which has an array in this buffer.
            /// 
            /// 3. `self.len` is less than or equal to `self.cap`, and both of
            ///    those values are strictly less than `new_cap`.
            /// 
            /// 4. The arrays in `self.ptr` are arranged like so:
            /// 
            ///    * Each array is laid out contiguously in the memory, in the
            ///      order which they are declared in the source code.
            /// 
            ///    * Each array contains `self.len` elements in them.
            ///
            /// 5. No external alias may read or write any part of the buffer
            ///    while this function is executing.
            ///
            /// Failure to satisfy **any** of these rules results in undefined
            /// behaviour.
            #[inline]
            unsafe fn resize(&mut self, new_ptr: NonNull<u8>, new_cap: usize) {{
                const fn align_up(x: isize, a: isize) -> isize {{
                    debug_assert!(a.is_power_of_two());
                    (x + a - 1) & !(a - 1)
                }}

                let mut cursor = 0;
                let mut old_cursor = 0;
                {resize_body}
            }}

            const fn buf_align(&self) -> usize {{
                let aligns = [
                    {alignments}
                ];
                let mut max = 1;
                let mut i = 0;
                while i < {num_fields} {{
                    if aligns[i] > max {{ max = aligns[i]; }}
                    i += 1;
                }}
                max
            }}

            {field_slice_methods}
        }}
        "#,
    )
    .parse()
    .unwrap()
}

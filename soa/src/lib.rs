extern crate proc_macro;

use proc_macro::TokenStream;

#[proc_macro_derive(Soa)]
pub fn soa_derive(input: TokenStream) -> TokenStream {
    todo!()
}

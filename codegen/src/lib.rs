extern crate proc_macro;

use std::ffi::CStr;

use proc_macro::TokenStream;
use quote::{__private::Span, quote};
use syn::{parse_macro_input, LitByteStr, LitStr};

mod interface;

#[proc_macro]
pub fn cstr(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as LitStr);
    let input = input.value();

    let mut bytes = input.into_bytes();
    bytes.push(0);

    // Check `bytes` statically so it can be skipped at runtime
    CStr::from_bytes_with_nul(&bytes).unwrap();

    let bytes = LitByteStr::new(&bytes, Span::call_site());

    let tokens = quote! {
        unsafe {
            std::ffi::CStr::from_bytes_with_nul_unchecked(#bytes)
        }
    };

    tokens.into()
}

#[proc_macro_attribute]
pub fn interface(_args: TokenStream, input: TokenStream) -> TokenStream {
    crate::interface::interface(input)
}

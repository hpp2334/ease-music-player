use service::parse_to_host;

mod service;

extern crate proc_macro;

#[proc_macro]
pub fn misty_to_host(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = proc_macro2::TokenStream::from(input);
    let output = parse_to_host(input);
    proc_macro::TokenStream::from(output)
}

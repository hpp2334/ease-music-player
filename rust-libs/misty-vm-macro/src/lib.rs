use async_task::parse_misty_async_task_derive;
use service::parse_misty_service;
use state::{parse_misty_state_derive, parse_misty_states};

mod async_task;
mod service;
mod state;

extern crate proc_macro;

#[proc_macro]
pub fn misty_states(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = proc_macro2::TokenStream::from(input);
    let output = parse_misty_states(input);
    proc_macro::TokenStream::from(output)
}

#[proc_macro_derive(MistyAsyncTask)]
pub fn misty_async_task_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = proc_macro2::TokenStream::from(input);
    let output = parse_misty_async_task_derive(input);
    proc_macro::TokenStream::from(output)
}

#[proc_macro]
pub fn misty_service(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = proc_macro2::TokenStream::from(input);
    let output = parse_misty_service(input);
    proc_macro::TokenStream::from(output)
}

#[proc_macro_derive(MistyState)]
pub fn misty_state_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = proc_macro2::TokenStream::from(input);
    let output = parse_misty_state_derive(input);
    proc_macro::TokenStream::from(output)
}

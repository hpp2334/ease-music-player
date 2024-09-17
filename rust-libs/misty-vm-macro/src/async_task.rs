use quote::quote;
use syn::{parse2, DeriveInput};

pub fn parse_misty_async_task_derive(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    let input = parse2::<DeriveInput>(input).unwrap();
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let output: proc_macro2::TokenStream = quote! {
        const _: () = {
            use misty_vm::client::MistyClientId;
            use misty_vm::async_task::*;
            use std::collections::HashMap;
            use std::sync::RwLock;

            impl #impl_generics MistyAsyncTaskTrait for #name #ty_generics #where_clause {

            }
        };
    };

    output
}

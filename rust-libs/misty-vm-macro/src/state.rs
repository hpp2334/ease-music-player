use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse2,
    punctuated::Punctuated,
    DeriveInput,
};

struct StatesStruct {
    states: Punctuated<syn::Type, syn::Token![,]>,
}

impl Parse for StatesStruct {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(StatesStruct {
            states: Punctuated::<syn::Type, syn::Token![,]>::parse_terminated(input)?,
        })
    }
}

pub fn parse_misty_states(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    let input = parse2::<StatesStruct>(input);
    if let Err(err) = input {
        panic!("parse misty state error: {}", err);
    }

    let input = input.unwrap();
    let state_types: Vec<syn::Type> = input.states.clone().into_iter().collect();

    let output: proc_macro2::TokenStream = quote! {
        {
            use misty_vm::client::MistyClientId;
            use misty_vm::states::{States};
            use std::collections::HashMap;
            use std::sync::RwLock;

            let mut states = States::new();
            #(
                states.register::<#state_types>();
            )*
            states
        }
    };

    output
}

pub fn parse_misty_state_derive(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    let input = parse2::<DeriveInput>(input).unwrap();
    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let output: proc_macro2::TokenStream = quote! {
        const _: () = {
            use misty_vm::client::MistyClientId;
            use misty_vm::states::{MistyStateTrait};
            use std::collections::HashMap;
            use std::sync::RwLock;

            impl #impl_generics MistyStateTrait for #name #ty_generics #where_clause {}
        };
    };

    output
}

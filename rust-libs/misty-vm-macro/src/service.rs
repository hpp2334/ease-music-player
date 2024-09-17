use proc_macro2::Ident;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    parse2,
};

struct ServiceStruct {
    marker_token: Ident,
    _comma: syn::Token![,],
    impl_token: Ident,
}

impl Parse for ServiceStruct {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(ServiceStruct {
            marker_token: input.parse()?,
            _comma: input.parse()?,
            impl_token: input.parse()?,
        })
    }
}

pub fn parse_misty_service(input: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    let input = parse2::<ServiceStruct>(input);
    if let Err(err) = input {
        panic!("parse misty service error: {}", err);
    }
    let input = input.unwrap();

    let marker_name = input.marker_token;
    let impl_name = input.impl_token;

    let output: proc_macro2::TokenStream = quote! {
        pub struct #marker_name {
            ptr: misty_vm::services::ServiceImplPtr<dyn #impl_name>,
        }
        const _: () = {
            use misty_vm::services::*;
            use std::sync::Arc;
            impl MistyServiceTrait for #marker_name {

            }
            impl #marker_name {
                pub fn new(service: impl #impl_name + 'static) -> Self {
                    Self {
                        ptr: ServiceImplPtr::Boxed(Box::new(service)),
                    }
                }
                pub fn new_with_box(service: Box<dyn #impl_name>) -> Self {
                    Self {
                        ptr: ServiceImplPtr::Boxed(service),
                    }
                }
                pub fn new_with_arc(service: Arc<dyn #impl_name>) -> Self {
                    Self {
                        ptr: ServiceImplPtr::Arc(service),
                    }
                }
            }
            impl std::ops::Deref for #marker_name {
                type Target = dyn #impl_name;

                fn deref(&self) -> &Self::Target {
                    match (&self.ptr) {
                        ServiceImplPtr::Boxed(ptr) => ptr.as_ref(),
                        ServiceImplPtr::Arc(ptr) => ptr.as_ref(),
                    }
                }
            }
        };
    };
    output
}

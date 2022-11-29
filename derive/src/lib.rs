extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, punctuated::Punctuated, DeriveInput};

#[proc_macro_derive(Query)]
pub fn derive_answer_fn(tokens: TokenStream) -> TokenStream {
    let input = parse_macro_input!(tokens as DeriveInput);

    match derive(input) {
        Ok(t) => t.into(),
        Err(e) => e.into_compile_error().into(),
    }
}

fn derive_fields(fields: &Punctuated<syn::Field, syn::token::Comma>) -> proc_macro2::TokenStream {
    let idents = fields.iter().map(|x| &x.ident);
    let colors = fields.iter().map(|x| x.colon_token);
    let tys = fields.iter().map(|x| &x.ty);
    quote!(
        #(#idents #colors <#tys as crate::sparql::FromQuery>::from_query(sol)?,)*
    )
}

fn derive(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let this = match input.data {
        syn::Data::Struct(syn::DataStruct {
            fields: syn::Fields::Named(fields),
            ..
        }) => {
            let fields = derive_fields(&fields.named);
            quote! {
                Self {
                    #fields
                }
            }
        }
        syn::Data::Struct(syn::DataStruct {
            fields: syn::Fields::Unnamed(fields),
            ..
        }) => {
            let fields = derive_fields(&fields.unnamed);
            quote! {
                Self (
                    #fields
                )
            }
        }
        syn::Data::Struct(syn::DataStruct {
            fields: syn::Fields::Unit,
            ..
        }) => quote! {
            Self
        },
        syn::Data::Enum(e) => {
            return Err(syn::parse::Error::new(
                e.enum_token.span,
                "Only struct is supported",
            ))
        }
        syn::Data::Union(u) => {
            return Err(syn::parse::Error::new(
                u.union_token.span,
                "Only struct is supported",
            ))
        }
    };

    let gen = input.generics;
    let ident = input.ident;

    let constraints = gen.type_params();

    let gen_constraints = quote! {
        #(#constraints: crate::sparql::FromQuery)*
    };

    Ok(quote!(
    impl #gen crate::sparql::FromQuery for #ident #gen where #gen_constraints {
        fn from_query(sol: Sol) -> Result<Self, &'static str> {
            use crate::sparql::FromQuery as _;
            Ok(#this)
        }
    }))
}

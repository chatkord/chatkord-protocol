//! `#[derive(Public)]` for `report`: an enum is public by its variants' names and the fields marked `#[public]`.

#![forbid(unsafe_code)]

use quote::{ToTokens, quote};
use syn::{Data, DeriveInput, Field, Fields, Variant, parse_macro_input};

/// Makes an enum `report::Public`: each variant is the word of its own name, and a variant with one field marked
/// `#[public]` is that word with the field's own fact inside, as `Status(404)`. An unmarked field is never read, so an
/// error that carries a path or an address still reports as its variant alone. A marked field whose own fact is a case
/// shows as that variant's word alone, so a value is never nested deeper than `Status(Refused)`.
#[proc_macro_derive(Public, attributes(public))]
pub fn public(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let Data::Enum(data) = &input.data else {
        return syn::Error::new_spanned(&input.ident, "Public is derived for an enum: a struct names no variant")
            .to_compile_error()
            .into();
    };
    let name = &input.ident;
    let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();
    let arms: Result<Vec<_>, _> = data.variants.iter().map(arm).collect();
    let arms = match arms {
        Ok(arms) => arms,
        Err(error) => return error.to_compile_error().into(),
    };
    quote! {
        impl #impl_generics ::report::Public for #name #type_generics #where_clause {
            fn fact(&self) -> ::report::Fact {
                match self {
                    #(#arms,)*
                }
            }
        }
    }
    .into()
}

fn is_public(field: &Field) -> bool {
    field.attrs.iter().any(|attribute| attribute.path().is_ident("public"))
}

fn arm(variant: &Variant) -> syn::Result<impl ToTokens> {
    let ident = &variant.ident;
    let text = ident.to_string();
    let word = quote!(::report::word!(#text));
    let marked: Vec<usize> =
        variant.fields.iter().enumerate().filter(|(_, field)| is_public(field)).map(|(at, _)| at).collect();
    let (pattern, shown) = match (marked.as_slice(), &variant.fields) {
        ([], Fields::Unit) => (quote!(Self::#ident), None),
        ([], Fields::Unnamed(_)) => (quote!(Self::#ident(..)), None),
        ([], Fields::Named(_)) => (quote!(Self::#ident { .. }), None),
        ([at], Fields::Unnamed(fields)) => {
            let slots = (0..fields.unnamed.len()).map(|slot| if slot == *at { quote!(shown) } else { quote!(_) });
            (quote!(Self::#ident(#(#slots),*)), Some(quote!(shown)))
        }
        ([at], Fields::Named(fields)) => {
            let field = fields.named.iter().nth(*at).and_then(|field| field.ident.as_ref());
            let field = field.ok_or_else(|| syn::Error::new_spanned(variant, "a named field has a name"))?;
            (quote!(Self::#ident { #field: shown, .. }), Some(quote!(shown)))
        }
        _ => return Err(syn::Error::new_spanned(variant, "a variant marks at most one field `#[public]`")),
    };
    Ok(match shown {
        None => quote!(#pattern => ::report::Fact::Word(#word)),
        Some(shown) => quote!(#pattern => ::report::Fact::case(#word, ::report::Public::fact(#shown))),
    })
}

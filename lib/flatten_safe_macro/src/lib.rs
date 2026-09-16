use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse::Parser, parse_macro_input, punctuated::Punctuated, Data, DeriveInput, Field, Fields,
    Lit, Meta, Token,
};

#[proc_macro_attribute]
pub fn flatten_safe(attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut input = parse_macro_input!(item as DeriveInput);
    let struct_name = input.ident.clone();

    let mut tag_key: String = "message".to_string();
    let mut tag_value: Option<String> = None;

    if !attr.is_empty() {
        let parser = Punctuated::<Meta, Token![,]>::parse_terminated;
        let metas = match parser.parse(attr) {
            Ok(m) => m,
            Err(e) => return e.to_compile_error().into(),
        };

        for meta in metas {
            if let Meta::NameValue(nv) = &meta {
                let ident = match nv.path.get_ident() {
                    Some(i) => i.to_string(),
                    None => continue,
                };
                let lit_str = match &nv.value {
                    syn::Expr::Lit(syn::ExprLit {
                        lit: Lit::Str(s), ..
                    }) => s.value(),
                    _ => {
                        return syn::Error::new_spanned(
                            &nv.value,
                            "flatten_safe: expected a string literal",
                        )
                        .to_compile_error()
                        .into();
                    }
                };

                match ident.as_str() {
                    "tag_key" => tag_key = lit_str,
                    "tag_value" => tag_value = Some(lit_str),
                    other => {
                        return syn::Error::new_spanned(
                            &nv.path,
                            format!("flatten_safe: unknown argument `{other}`"),
                        )
                        .to_compile_error()
                        .into();
                    }
                }
            }
        }
    }

    let fields = match &mut input.data {
        Data::Struct(data) => match &mut data.fields {
            Fields::Named(named) => &mut named.named,
            _ => {
                return syn::Error::new_spanned(
                    &input,
                    "flatten_safe only supports structs with named fields",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(&input, "flatten_safe only supports structs")
                .to_compile_error()
                .into();
        }
    };

    let mut flatten_field: Option<Field> = None;
    let mut other_fields: Vec<Field> = Vec::new();

    for field in fields.iter_mut() {
        let is_flatten = field.attrs.iter().any(|a| a.path().is_ident("flatten"));
        field.attrs.retain(|a| !a.path().is_ident("flatten"));
        if is_flatten {
            field.attrs.push(syn::parse_quote!(#[serde(flatten)]));
            flatten_field = Some(field.clone());
        } else {
            other_fields.push(field.clone());
        }
    }

    let flatten_field = match flatten_field {
        Some(f) => f,
        None => {
            return syn::Error::new_spanned(
                &input,
                "flatten_safe requires exactly one field marked #[flatten]",
            )
            .to_compile_error()
            .into();
        }
    };

    input
        .attrs
        .push(syn::parse_quote!(#[derive(::serde::Deserialize)]));
    input
        .attrs
        .push(syn::parse_quote!(#[serde(try_from = "::serde_json::Value")]));

    let flat_ident = flatten_field.ident.clone().unwrap();
    let flat_ty = flatten_field.ty.clone();

    let other_idents: Vec<_> = other_fields
        .iter()
        .map(|f| f.ident.clone().unwrap())
        .collect();
    let other_tys: Vec<_> = other_fields.iter().map(|f| f.ty.clone()).collect();
    let other_names: Vec<_> = other_idents.iter().map(|i| i.to_string()).collect();

    let flat_value_expr = if let Some(tag_val) = &tag_value {
        quote! {
            {
                let mut patched = value.clone();
                if let ::serde_json::Value::Object(map) = &mut patched {
                    map.insert(
                        #tag_key.to_string(),
                        ::serde_json::Value::String(#tag_val.to_string()),
                    );
                }
                patched
            }
        }
    } else {
        quote! { value.clone() }
    };

    let try_from_impl = quote! {
        impl ::std::convert::TryFrom<::serde_json::Value> for #struct_name {
            type Error = ::serde_json::Error;

            fn try_from(value: ::serde_json::Value) -> ::std::result::Result<Self, Self::Error> {
                let #flat_ident: #flat_ty = ::serde_json::from_value(#flat_value_expr)?;
                #(
                    let #other_idents: #other_tys = ::serde_json::from_value(
                        value.get(#other_names)
                            .cloned()
                            .unwrap_or(::serde_json::Value::Null)
                    )?;
                )*
                Ok(#struct_name {
                    #flat_ident,
                    #( #other_idents, )*
                })
            }
        }
    };

    quote! {
        #input
        #try_from_impl
    }
    .into()
}

use proc_macro::TokenStream;
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{
    parse_macro_input, GenericArgument, ImplItem, ItemImpl, ItemStruct, Meta, PathArguments, Token,
};

fn lib_path() -> proc_macro2::TokenStream {
    match crate_name("network_abstraction_lib") {
        Ok(FoundCrate::Itself) => quote!(crate),
        Ok(FoundCrate::Name(name)) => {
            let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
            quote!(::#ident)
        }
        Err(_) => quote!(::network_abstraction_lib),
    }
}

enum KeyOverride {
    None,
    Literal(String),
    SnakeCase,
}

fn parse_key_override(attr: TokenStream) -> syn::Result<KeyOverride> {
    if attr.is_empty() {
        return Ok(KeyOverride::None);
    }

    let metas = Punctuated::<Meta, Token![,]>::parse_terminated.parse(attr)?;
    let mut result = KeyOverride::None;

    for meta in metas {
        let new = match &meta {
            Meta::NameValue(nv) if nv.path.is_ident("name") => {
                if let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(s),
                    ..
                }) = &nv.value
                {
                    KeyOverride::Literal(s.value())
                } else {
                    return Err(syn::Error::new_spanned(
                        nv,
                        "expected `name = \"...\"` with a string literal",
                    ));
                }
            }
            Meta::Path(p) if p.is_ident("snake_case") => KeyOverride::SnakeCase,
            _ => {
                return Err(syn::Error::new_spanned(
                    meta,
                    "unsupported attribute; expected `name = \"...\"` or `snake_case`",
                ))
            }
        };

        if !matches!(result, KeyOverride::None) {
            return Err(syn::Error::new_spanned(
                meta,
                "only one of `name` or `snake_case` may be specified",
            ));
        }
        result = new;
    }

    Ok(result)
}

fn to_snake_case(s: &str) -> String {
    let mut out = String::new();
    let mut prev_lower_or_digit = false;
    for c in s.chars() {
        if c.is_uppercase() {
            if prev_lower_or_digit {
                out.push('_');
            }
            out.extend(c.to_lowercase());
            prev_lower_or_digit = false;
        } else {
            out.push(c);
            prev_lower_or_digit = c.is_lowercase() || c.is_numeric();
        }
    }
    out
}

fn type_key_method(
    self_ty: &syn::Type,
    key_override: &KeyOverride,
) -> syn::Result<proc_macro2::TokenStream> {
    match key_override {
        KeyOverride::None => Ok(quote!()),
        KeyOverride::Literal(name) => Ok(quote! {
            fn type_key() -> String { #name.to_string() }
        }),
        KeyOverride::SnakeCase => {
            let ident_str = match self_ty {
                syn::Type::Path(tp) => tp
                    .path
                    .segments
                    .last()
                    .map(|s| s.ident.to_string())
                    .ok_or_else(|| syn::Error::new_spanned(self_ty, "expected a named type"))?,
                _ => {
                    return Err(syn::Error::new_spanned(
                        self_ty,
                        "`snake_case` requires a named struct type",
                    ))
                }
            };
            let snake = to_snake_case(&ident_str);
            Ok(quote! {
                fn type_key() -> String { #snake.to_string() }
            })
        }
    }
}

#[proc_macro_attribute]
pub fn typed_request(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_impl = match syn::parse::<ItemImpl>(item.clone()) {
        Ok(imp) => imp,
        Err(e) => {
            return e.to_compile_error().into();
        }
    };
    let key_override = match parse_key_override(attr) {
        Ok(k) => k,
        Err(e) => return e.to_compile_error().into(),
    };
    match expand(&input_impl, &key_override) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn expand(
    input_impl: &ItemImpl,
    key_override: &KeyOverride,
) -> syn::Result<proc_macro2::TokenStream> {
    let lib = lib_path();
    let self_ty = &input_impl.self_ty;

    let trait_path = &input_impl
        .trait_
        .as_ref()
        .ok_or_else(|| {
            syn::Error::new_spanned(
                self_ty,
                "#[typed_request] must be applied to `impl RouteInput<S> for Input { .. }`, not an inherent impl",
            )
        })?
        .1;

    let last_seg = trait_path.segments.last().ok_or_else(|| {
        syn::Error::new_spanned(
            trait_path,
            "expected a trait path, e.g. RouteInput<Arc<State>>",
        )
    })?;

    if last_seg.ident != "RouteInput" {
        return Err(syn::Error::new_spanned(
            last_seg,
            "#[typed_request] only applies to RouteInput impls",
        ));
    }

    let state_ty = match &last_seg.arguments {
        PathArguments::AngleBracketed(args) => args
            .args
            .iter()
            .find_map(|a| match a {
                GenericArgument::Type(t) => Some(t.clone()),
                _ => None,
            })
            .ok_or_else(|| {
                syn::Error::new_spanned(
                    last_seg,
                    "RouteInput needs a state type argument, e.g. RouteInput<Arc<State>>",
                )
            })?,
        _ => {
            return Err(syn::Error::new_spanned(
                last_seg,
                "expected RouteInput<S> with an angle-bracketed state type",
            ))
        }
    };

    let output_ty = input_impl
        .items
        .iter()
        .find_map(|it| match it {
            ImplItem::Type(t) if t.ident == "Output" => Some(t.ty.clone()),
            _ => None,
        })
        .ok_or_else(|| {
            syn::Error::new_spanned(
                self_ty,
                "#[typed_request] requires `type Output = ...;` already written in the impl body",
            )
        })?;

    let type_key_method = type_key_method(self_ty.as_ref(), key_override)?;

    Ok(quote! {
        impl #trait_path for #self_ty {
            type Output = #output_ty;

            fn slot() -> &'static ::std::sync::OnceLock<::std::sync::Arc<dyn #lib::typed::TypedHandler<#state_ty, Input = #self_ty, Output = #output_ty>>> {
                static SLOT: ::std::sync::OnceLock<::std::sync::Arc<dyn #lib::typed::TypedHandler<#state_ty, Input = #self_ty, Output = #output_ty>>> = ::std::sync::OnceLock::new();
                &SLOT
            }

            #type_key_method
        }
    })
}

#[proc_macro_attribute]
pub fn register_output(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemStruct);
    let ident = &input.ident;
    let lib = lib_path();

    quote! {
        #input

        impl #lib::typed::TaggedOutput for #ident {
            fn tag(&self) -> &'static str {
                stringify!(#ident)
            }
        }

        #lib::inventory::submit! {
            #lib::typed::OutputRegistration {
                id: stringify!(#ident),
                deser: |d| {
                    Ok(Box::new(
                        #lib::erased_serde::deserialize::<#ident>(d)?
                    ))
                },
            }
        }
    }
    .into()
}

#[proc_macro_attribute]
pub fn typed_stream_request(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_impl = match syn::parse::<ItemImpl>(item.clone()) {
        Ok(imp) => imp,
        Err(e) => {
            return e.to_compile_error().into();
        }
    };
    let key_override = match parse_key_override(attr) {
        Ok(k) => k,
        Err(e) => return e.to_compile_error().into(),
    };
    match expand_stream(&input_impl, &key_override) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

fn expand_stream(
    input_impl: &ItemImpl,
    key_override: &KeyOverride,
) -> syn::Result<proc_macro2::TokenStream> {
    let lib = lib_path();
    let self_ty = &input_impl.self_ty;

    let trait_path = &input_impl
        .trait_
        .as_ref()
        .ok_or_else(|| {
            syn::Error::new_spanned(
                self_ty,
                "#[typed_stream_request] must be applied to `impl StreamRouteInput<S> for Input { .. }`, not an inherent impl",
            )
        })?
        .1;

    let last_seg = trait_path.segments.last().ok_or_else(|| {
        syn::Error::new_spanned(
            trait_path,
            "expected a trait path, e.g. StreamRouteInput<Arc<State>>",
        )
    })?;

    if last_seg.ident != "StreamRouteInput" {
        return Err(syn::Error::new_spanned(
            last_seg,
            "#[typed_stream_request] only applies to StreamRouteInput impls",
        ));
    }

    let state_ty = match &last_seg.arguments {
        PathArguments::AngleBracketed(args) => args
            .args
            .iter()
            .find_map(|a| match a {
                GenericArgument::Type(t) => Some(t.clone()),
                _ => None,
            })
            .ok_or_else(|| {
                syn::Error::new_spanned(
                    last_seg,
                    "StreamRouteInput needs a state type argument, e.g. StreamRouteInput<Arc<State>>",
                )
            })?,
        _ => {
            return Err(syn::Error::new_spanned(
                last_seg,
                "expected StreamRouteInput<S> with an angle-bracketed state type",
            ))
        }
    };

    let item_ty = input_impl
        .items
        .iter()
        .find_map(|it| match it {
            ImplItem::Type(t) if t.ident == "Item" => Some(t.ty.clone()),
            _ => None,
        })
        .ok_or_else(|| {
            syn::Error::new_spanned(
                self_ty,
                "#[typed_stream_request] requires `type Item = ...;` already written in the impl body",
            )
        })?;

    let type_key_method = type_key_method(self_ty.as_ref(), key_override)?;

    Ok(quote! {
        impl #trait_path for #self_ty {
            type Item = #item_ty;

            fn slot() -> &'static ::std::sync::OnceLock<::std::sync::Arc<dyn #lib::typed_stream::TypedStreamHandler<#state_ty, Input = #self_ty, Item = #item_ty>>> {
                static SLOT: ::std::sync::OnceLock<::std::sync::Arc<dyn #lib::typed_stream::TypedStreamHandler<#state_ty, Input = #self_ty, Item = #item_ty>>> = ::std::sync::OnceLock::new();
                &SLOT
            }

            #type_key_method
        }
    })
}

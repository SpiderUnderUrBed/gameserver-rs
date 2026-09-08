use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, GenericArgument, ImplItem, ItemImpl, PathArguments};

<<<<<<< HEAD
#[proc_macro_attribute]
pub fn typed_request(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_impl = parse_macro_input!(item as ItemImpl);

    match expand(&input_impl) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
=======
    #[proc_macro_attribute]
    pub fn typed_request(_attr: TokenStream, item: TokenStream) -> TokenStream {
        let input_impl = match syn::parse::<ItemImpl>(item.clone()) {
            Ok(imp) => imp,
            Err(e) => {
                return item;
            }
        };
        match expand(&input_impl) {
            Ok(tokens) => tokens.into(),
            Err(e) => e.to_compile_error().into(),
        }
>>>>>>> c5057e3 (Fixed an error popping up with RA for the macro)
    }
}

fn expand(input_impl: &ItemImpl) -> syn::Result<proc_macro2::TokenStream> {
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

<<<<<<< HEAD
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
=======
        Ok(quote! {
            impl #trait_path for #self_ty {
                type Output = #output_ty;
>>>>>>> c5057e3 (Fixed an error popping up with RA for the macro)

    Ok(quote! {
        impl #trait_path for #self_ty {
            type Output = #output_ty;

            fn slot() -> &'static ::std::sync::OnceLock<
                ::std::sync::Arc<
                    dyn crate::typed::TypedHandler<#state_ty, Input = #self_ty, Output = #output_ty>
                >
            > {
                static SLOT: ::std::sync::OnceLock<
                    ::std::sync::Arc<
                        dyn crate::typed::TypedHandler<#state_ty, Input = #self_ty, Output = #output_ty>
                    >
                > = ::std::sync::OnceLock::new();
                &SLOT
            }
        }
    })
}

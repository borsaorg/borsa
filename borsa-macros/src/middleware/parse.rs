use proc_macro::TokenStream;
use proc_macro2::{Ident, Span};
use quote::quote;
use syn::{ItemImpl, Meta, MetaNameValue, Path, Token, parse_macro_input, punctuated::Punctuated};

use proc_macro_crate::{FoundCrate, crate_name};

fn resolve_borsa_core_path() -> Path {
    // Allow using either dependency or the local crate name
    let found = crate_name("borsa-core").unwrap_or(FoundCrate::Itself);
    match found {
        FoundCrate::Itself => syn::parse_quote! { borsa_core },
        FoundCrate::Name(name) => {
            let ident = Ident::new(&name, Span::call_site());
            syn::parse_quote! { #ident }
        }
    }
}

// Filesystem source parsing removed. Codegen is driven by borsa-core macros.

// Deleted brittle env/relative-path scanning helpers.

// Deleted AST discovery helpers.

fn parse_inner_ident(args: Punctuated<Meta, Token![,]>) -> (Ident, Option<String>) {
    let mut inner: Option<Ident> = None;
    for meta in args {
        match meta {
            // legacy: previously supported `pre_call = "..."`; ignore now
            Meta::Path(p) => {
                if let Some(ident) = p.get_ident() {
                    inner = Some(ident.clone());
                }
            }
            Meta::NameValue(MetaNameValue { .. }) | Meta::List(_) => {}
        }
    }
    let inner_ident = inner.expect("delegate macro requires the inner field ident as first arg, e.g., #[delegate_connector(inner)]");
    (inner_ident, None)
}

// Accessor name generation no longer needed here; implemented in core macros.

// Validation removed; core macros are now the single source of truth.

pub fn delegate_connector_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr with Punctuated::<Meta, Token![,]>::parse_terminated);
    let input_impl = parse_macro_input!(item as ItemImpl);
    let (inner_ident, _pre_call) = parse_inner_ident(args);

    let borsa_core = resolve_borsa_core_path();

    // We expect an inherent impl block on the target type; we will append another impl for BorsaConnector.
    let self_ty = *input_impl.self_ty.clone();

    // Generate delegation for name/vendor/supports_kind and dynamic as_* methods via core macros
    let expanded = quote! {
        #input_impl

        impl #borsa_core::connector::BorsaConnector for #self_ty {
            fn name(&self) -> &'static str { self.#inner_ident.name() }
            fn vendor(&self) -> &'static str { self.#inner_ident.vendor() }
            fn supports_kind(&self, kind: #borsa_core::AssetKind) -> bool { self.#inner_ident.supports_kind(kind) }
            #borsa_core::borsa_connector_accessors!(#inner_ident);
        }
    };

    expanded.into()
}

// All per-provider dispatch moved to core macros.

pub fn delegate_all_providers_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr with Punctuated::<Meta, Token![,]>::parse_terminated);
    let input_impl = parse_macro_input!(item as ItemImpl);
    let (inner_ident, _pre_call) = parse_inner_ident(args);

    let borsa_core = resolve_borsa_core_path();
    let self_ty = *input_impl.self_ty.clone();

    let expanded = quote! {
        #input_impl
        #borsa_core::borsa_delegate_provider_impls!(#self_ty, #inner_ident);
    };

    expanded.into()
}

// Removed many per-provider helpers; provided by core macro.

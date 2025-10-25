mod middleware;

use middleware::{delegate_all_providers_impl, delegate_connector_impl};

#[proc_macro_attribute]
pub fn delegate_connector(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    delegate_connector_impl(attr, item)
}

#[proc_macro_attribute]
pub fn delegate_all_providers(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    delegate_all_providers_impl(attr, item)
}

use proc_macro::TokenStream;
use proc_macro2::{Ident, Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{
    Expr, GenericArgument, Item, ItemImpl, Lit, Meta, MetaNameValue, Path, ReturnType, Token,
    TraitItem, TraitItemFn, Type, parse_macro_input, punctuated::Punctuated,
};

use proc_macro_crate::{FoundCrate, crate_name};
use std::collections::HashSet;

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

/// Exhaustive list of provider traits in `BorsaConnector`.
///
/// This is the **source of truth** for code generation. When adding a new provider:
/// 1. Add the trait to `borsa-core/src/connector.rs`
/// 2. Add the trait name here
/// 3. Add a corresponding `gen_*_impl` function below
///
/// The macro will optionally validate this list against `connector.rs` at compile time
/// if the file is found. If validation fails, warnings are emitted but compilation
/// continues (allowing external crates to use these macros without filesystem access).
const KNOWN_PROVIDERS: &[&str] = &[
    "HistoryProvider",
    "QuoteProvider",
    "EarningsProvider",
    "IncomeStatementProvider",
    "BalanceSheetProvider",
    "CashflowProvider",
    "CalendarProvider",
    "RecommendationsProvider",
    "RecommendationsSummaryProvider",
    "UpgradesDowngradesProvider",
    "AnalystPriceTargetProvider",
    "MajorHoldersProvider",
    "InstitutionalHoldersProvider",
    "MutualFundHoldersProvider",
    "InsiderTransactionsProvider",
    "InsiderRosterHoldersProvider",
    "NetSharePurchaseActivityProvider",
    "ProfileProvider",
    "IsinProvider",
    "SearchProvider",
    "EsgProvider",
    "NewsProvider",
    "OptionsExpirationsProvider",
    "OptionChainProvider",
    "StreamProvider",
];

fn find_borsa_core_connector_rs() -> Option<std::path::PathBuf> {
    if let Ok(override_path) = std::env::var("BORSA_CORE_CONNECTOR_RS") {
        let p = std::path::PathBuf::from(override_path);
        if p.exists() {
            return Some(p);
        }
    }
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        let base = std::path::PathBuf::from(manifest);
        for up in [1, 2, 3, 4] {
            let mut p = base.clone();
            for _ in 0..up {
                p.push("..");
            }
            p.push("borsa-core/src/connector.rs");
            if p.exists() {
                return Some(p);
            }
        }
    }
    None
}

fn load_connector_ast() -> Option<syn::File> {
    let path = find_borsa_core_connector_rs()?;
    let content = std::fs::read_to_string(&path).ok()?;
    syn::parse_file(&content).ok()
}

fn discover_accessors(file: &syn::File) -> Vec<(Ident, Ident)> {
    // Returns pairs: (accessor_fn_ident, provider_trait_ident)
    for item in &file.items {
        if let Item::Trait(tr) = item
            && tr.ident == "BorsaConnector"
        {
            let mut found: Vec<(Ident, Ident)> = Vec::new();
            for ti in &tr.items {
                if let TraitItem::Fn(TraitItemFn { sig, .. }) = ti {
                    let name = sig.ident.to_string();
                    if name.starts_with("as_") && name.ends_with("_provider") {
                        // Parse return type: Option<&dyn Trait>
                        if let ReturnType::Type(_, ty) = &sig.output
                            && let Type::Path(tp) = ty.as_ref()
                        {
                            // Option<...>
                            if let Some(seg) = tp.path.segments.last()
                                && seg.ident == "Option"
                                && let syn::PathArguments::AngleBracketed(ab) = &seg.arguments
                                && let Some(GenericArgument::Type(Type::Reference(r))) =
                                    ab.args.first()
                            {
                                let elem = r.elem.as_ref();
                                match elem {
                                    Type::TraitObject(to) => {
                                        // dyn Trait
                                        let mut provider_ident: Option<Ident> = None;
                                        for b in &to.bounds {
                                            if let syn::TypeParamBound::Trait(tb) = b
                                                && let Some(ident) =
                                                    tb.path.segments.last().map(|s| s.ident.clone())
                                            {
                                                provider_ident = Some(ident);
                                                break;
                                            }
                                        }
                                        if let Some(pid) = provider_ident {
                                            found.push((sig.ident.clone(), pid));
                                        }
                                    }
                                    Type::Path(p2) => {
                                        if let Some(id2) =
                                            p2.path.segments.last().map(|s| s.ident.clone())
                                        {
                                            found.push((sig.ident.clone(), id2));
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
            return found;
        }
    }
    Vec::new()
}

fn parse_inner_ident(args: Punctuated<Meta, Token![,]>) -> (Ident, Option<String>) {
    let mut inner: Option<Ident> = None;
    let mut pre_call: Option<String> = None;
    for meta in args {
        match meta {
            Meta::NameValue(MetaNameValue { path, value, .. }) if path.is_ident("pre_call") => {
                if let Expr::Lit(expr_lit) = value
                    && let Lit::Str(s) = expr_lit.lit
                {
                    pre_call = Some(s.value());
                }
            }
            Meta::Path(p) => {
                if let Some(ident) = p.get_ident() {
                    inner = Some(ident.clone());
                }
            }
            _ => {}
        }
    }
    let inner_ident = inner.expect("delegate macro requires the inner field ident as first arg, e.g., #[delegate_connector(inner)]");
    (inner_ident, pre_call)
}

fn provider_to_accessor_name(provider: &str) -> String {
    let without_provider = provider.strip_suffix("Provider").unwrap_or(provider);
    let snake_case = without_provider
        .chars()
        .enumerate()
        .flat_map(|(i, c)| {
            if i > 0 && c.is_uppercase() {
                vec!['_', c.to_ascii_lowercase()]
            } else {
                vec![c.to_ascii_lowercase()]
            }
        })
        .collect::<String>();
    format!("as_{snake_case}_provider")
}

/// Optional compile-time validation: compare `KNOWN_PROVIDERS` against connector.rs.
///
/// This validation is best-effort and gracefully degrades:
/// - If `connector.rs` is not found (e.g., in external crates), no validation occurs
/// - If validation detects drift, warnings are emitted but compilation continues
/// - In the monorepo, warnings help catch when a provider is added/removed
///
/// This makes the macro robust for external users while providing helpful feedback
/// for maintainers in the monorepo environment.
fn validate_providers_against_file() {
    let Some(file) = load_connector_ast() else {
        return;
    };

    let discovered = discover_accessors(&file);
    let discovered_providers: HashSet<String> = discovered
        .iter()
        .map(|(_accessor, provider)| provider.to_string())
        .collect();

    let known_set: HashSet<String> = KNOWN_PROVIDERS.iter().map(|s| (*s).to_string()).collect();

    let missing_in_macro: Vec<_> = discovered_providers.difference(&known_set).collect();
    let missing_in_file: Vec<_> = known_set.difference(&discovered_providers).collect();

    if !missing_in_macro.is_empty() {
        eprintln!(
            "cargo:warning=borsa-macros: Provider(s) in connector.rs but not in KNOWN_PROVIDERS: {missing_in_macro:?}"
        );
        eprintln!(
            "cargo:warning=Please update KNOWN_PROVIDERS in borsa-macros/src/middleware/parse.rs"
        );
    }

    if !missing_in_file.is_empty() {
        eprintln!(
            "cargo:warning=borsa-macros: Provider(s) in KNOWN_PROVIDERS but not in connector.rs: {missing_in_file:?}"
        );
        eprintln!("cargo:warning=This may indicate a typo or outdated list");
    }
}

pub fn delegate_connector_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr with Punctuated::<Meta, Token![,]>::parse_terminated);
    let input_impl = parse_macro_input!(item as ItemImpl);
    let (inner_ident, _pre_call) = parse_inner_ident(args);

    let borsa_core = resolve_borsa_core_path();

    // We expect an inherent impl block on the target type; we will append another impl for BorsaConnector.
    let self_ty = *input_impl.self_ty.clone();

    // Attempt optional validation: compare KNOWN_PROVIDERS with connector.rs
    validate_providers_against_file();

    // Generate accessor methods from KNOWN_PROVIDERS (source of truth)
    let mut accessor_methods: Vec<TokenStream2> = Vec::new();
    for provider_name in KNOWN_PROVIDERS {
        let provider_ident = Ident::new(provider_name, Span::call_site());
        let accessor_name = provider_to_accessor_name(provider_name);
        let accessor_ident = Ident::new(&accessor_name, Span::call_site());

        let method = quote! {
            fn #accessor_ident(&self) -> Option<&dyn #borsa_core::connector::#provider_ident> {
                if self.#inner_ident.#accessor_ident().is_some() {
                    Some(self as &dyn #borsa_core::connector::#provider_ident)
                } else {
                    None
                }
            }
        };
        accessor_methods.push(method);
    }

    // Generate delegation for name/vendor/supports_kind and dynamic as_* methods
    let expanded = quote! {
        #input_impl

        impl #borsa_core::connector::BorsaConnector for #self_ty {
            fn name(&self) -> &'static str { self.#inner_ident.name() }
            fn vendor(&self) -> &'static str { self.#inner_ident.vendor() }
            fn supports_kind(&self, kind: #borsa_core::AssetKind) -> bool { self.#inner_ident.supports_kind(kind) }
            #(#accessor_methods)*
        }
    };

    expanded.into()
}

/// Dispatch to the appropriate generator function based on provider name.
fn generate_provider_impl(
    provider_name: &str,
    borsa_core: &Path,
    self_ty: &Type,
    inner: &Ident,
    pre: &TokenStream2,
) -> TokenStream2 {
    match provider_name {
        "HistoryProvider" => gen_history_impl(borsa_core, self_ty, inner, pre),
        "QuoteProvider" => gen_quote_impl(borsa_core, self_ty, inner, pre),
        "EarningsProvider" => gen_earnings_impl(borsa_core, self_ty, inner, pre),
        "IncomeStatementProvider" => gen_income_stmt_impl(borsa_core, self_ty, inner, pre),
        "BalanceSheetProvider" => gen_balance_sheet_impl(borsa_core, self_ty, inner, pre),
        "CashflowProvider" => gen_cashflow_impl(borsa_core, self_ty, inner, pre),
        "CalendarProvider" => gen_calendar_impl(borsa_core, self_ty, inner, pre),
        "RecommendationsProvider" => gen_recommendations_impl(borsa_core, self_ty, inner, pre),
        "RecommendationsSummaryProvider" => {
            gen_recommendations_summary_impl(borsa_core, self_ty, inner, pre)
        }
        "UpgradesDowngradesProvider" => gen_upgrades_impl(borsa_core, self_ty, inner, pre),
        "AnalystPriceTargetProvider" => gen_price_target_impl(borsa_core, self_ty, inner, pre),
        "MajorHoldersProvider" => gen_major_holders_impl(borsa_core, self_ty, inner, pre),
        "InstitutionalHoldersProvider" => {
            gen_institutional_holders_impl(borsa_core, self_ty, inner, pre)
        }
        "MutualFundHoldersProvider" => {
            gen_mutual_fund_holders_impl(borsa_core, self_ty, inner, pre)
        }
        "InsiderTransactionsProvider" => {
            gen_insider_transactions_impl(borsa_core, self_ty, inner, pre)
        }
        "InsiderRosterHoldersProvider" => gen_insider_roster_impl(borsa_core, self_ty, inner, pre),
        "NetSharePurchaseActivityProvider" => {
            gen_net_share_purchase_impl(borsa_core, self_ty, inner, pre)
        }
        "ProfileProvider" => gen_profile_impl(borsa_core, self_ty, inner, pre),
        "IsinProvider" => gen_isin_impl(borsa_core, self_ty, inner, pre),
        "SearchProvider" => gen_search_impl(borsa_core, self_ty, inner, pre),
        "EsgProvider" => gen_esg_impl(borsa_core, self_ty, inner, pre),
        "NewsProvider" => gen_news_impl(borsa_core, self_ty, inner, pre),
        "OptionsExpirationsProvider" => {
            gen_options_expirations_impl(borsa_core, self_ty, inner, pre)
        }
        "OptionChainProvider" => gen_option_chain_impl(borsa_core, self_ty, inner, pre),
        "StreamProvider" => gen_stream_impl(borsa_core, self_ty, inner, pre),
        _ => panic!("Unknown provider in KNOWN_PROVIDERS: {provider_name}"),
    }
}

pub fn delegate_all_providers_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr with Punctuated::<Meta, Token![,]>::parse_terminated);
    let input_impl = parse_macro_input!(item as ItemImpl);
    let (inner_ident, pre_call) = parse_inner_ident(args);
    let pre_call_ts: TokenStream2 = pre_call.map_or_else(
        || quote! {},
        |s| syn::parse_str::<TokenStream2>(&s).expect("invalid pre_call expression"),
    );

    let borsa_core = resolve_borsa_core_path();
    let self_ty = *input_impl.self_ty.clone();

    // Attempt optional validation: compare KNOWN_PROVIDERS with connector.rs
    validate_providers_against_file();

    // Generate implementations dynamically from KNOWN_PROVIDERS
    let impls: Vec<TokenStream2> = KNOWN_PROVIDERS
        .iter()
        .map(|provider_name| {
            generate_provider_impl(
                provider_name,
                &borsa_core,
                &self_ty,
                &inner_ident,
                &pre_call_ts,
            )
        })
        .collect();

    let expanded = quote! {
        #input_impl
        #(#impls)*
    };

    expanded.into()
}

fn gen_history_impl(
    borsa_core: &Path,
    self_ty: &Type,
    inner: &Ident,
    pre: &TokenStream2,
) -> TokenStream2 {
    quote! {
        #[async_trait::async_trait]
        impl #borsa_core::connector::HistoryProvider for #self_ty {
            async fn history(&self, instrument: &#borsa_core::Instrument, req: #borsa_core::HistoryRequest) -> Result<#borsa_core::HistoryResponse, #borsa_core::BorsaError> {
                #pre
                let inner = self.#inner.as_history_provider().ok_or_else(|| #borsa_core::BorsaError::unsupported("history"))?;
                inner.history(instrument, req).await
            }
            fn supported_history_intervals(&self, kind: #borsa_core::AssetKind) -> &'static [#borsa_core::Interval] {
                if let Some(inner) = self.#inner.as_history_provider() { inner.supported_history_intervals(kind) } else { &[] }
            }
        }
    }
}

fn gen_quote_impl(
    borsa_core: &Path,
    self_ty: &Type,
    inner: &Ident,
    pre: &TokenStream2,
) -> TokenStream2 {
    quote! {
        #[async_trait::async_trait]
        impl #borsa_core::connector::QuoteProvider for #self_ty {
            async fn quote(&self, instrument: &#borsa_core::Instrument) -> Result<#borsa_core::Quote, #borsa_core::BorsaError> {
                #pre
                let inner = self.#inner.as_quote_provider().ok_or_else(|| #borsa_core::BorsaError::unsupported("quote"))?;
                inner.quote(instrument).await
            }
        }
    }
}

fn gen_earnings_impl(
    borsa_core: &Path,
    self_ty: &Type,
    inner: &Ident,
    pre: &TokenStream2,
) -> TokenStream2 {
    quote! {
        #[async_trait::async_trait]
        impl #borsa_core::connector::EarningsProvider for #self_ty {
            async fn earnings(&self, instrument: &#borsa_core::Instrument) -> Result<#borsa_core::Earnings, #borsa_core::BorsaError> {
                #pre
                let inner = self.#inner.as_earnings_provider().ok_or_else(|| #borsa_core::BorsaError::unsupported("earnings"))?;
                inner.earnings(instrument).await
            }
        }
    }
}

fn gen_income_stmt_impl(
    borsa_core: &Path,
    self_ty: &Type,
    inner: &Ident,
    pre: &TokenStream2,
) -> TokenStream2 {
    quote! {
        #[async_trait::async_trait]
        impl #borsa_core::connector::IncomeStatementProvider for #self_ty {
            async fn income_statement(&self, instrument: &#borsa_core::Instrument, quarterly: bool) -> Result<Vec<#borsa_core::IncomeStatementRow>, #borsa_core::BorsaError> {
                #pre
                let inner = self.#inner.as_income_statement_provider().ok_or_else(|| #borsa_core::BorsaError::unsupported("income_statement"))?;
                inner.income_statement(instrument, quarterly).await
            }
        }
    }
}

fn gen_balance_sheet_impl(
    borsa_core: &Path,
    self_ty: &Type,
    inner: &Ident,
    pre: &TokenStream2,
) -> TokenStream2 {
    quote! {
        #[async_trait::async_trait]
        impl #borsa_core::connector::BalanceSheetProvider for #self_ty {
            async fn balance_sheet(&self, instrument: &#borsa_core::Instrument, quarterly: bool) -> Result<Vec<#borsa_core::BalanceSheetRow>, #borsa_core::BorsaError> {
                #pre
                let inner = self.#inner.as_balance_sheet_provider().ok_or_else(|| #borsa_core::BorsaError::unsupported("balance_sheet"))?;
                inner.balance_sheet(instrument, quarterly).await
            }
        }
    }
}

fn gen_cashflow_impl(
    borsa_core: &Path,
    self_ty: &Type,
    inner: &Ident,
    pre: &TokenStream2,
) -> TokenStream2 {
    quote! {
        #[async_trait::async_trait]
        impl #borsa_core::connector::CashflowProvider for #self_ty {
            async fn cashflow(&self, instrument: &#borsa_core::Instrument, quarterly: bool) -> Result<Vec<#borsa_core::CashflowRow>, #borsa_core::BorsaError> {
                #pre
                let inner = self.#inner.as_cashflow_provider().ok_or_else(|| #borsa_core::BorsaError::unsupported("cashflow"))?;
                inner.cashflow(instrument, quarterly).await
            }
        }
    }
}

fn gen_calendar_impl(
    borsa_core: &Path,
    self_ty: &Type,
    inner: &Ident,
    pre: &TokenStream2,
) -> TokenStream2 {
    quote! {
        #[async_trait::async_trait]
        impl #borsa_core::connector::CalendarProvider for #self_ty {
            async fn calendar(&self, instrument: &#borsa_core::Instrument) -> Result<#borsa_core::Calendar, #borsa_core::BorsaError> {
                #pre
                let inner = self.#inner.as_calendar_provider().ok_or_else(|| #borsa_core::BorsaError::unsupported("calendar"))?;
                inner.calendar(instrument).await
            }
        }
    }
}

fn gen_recommendations_impl(
    borsa_core: &Path,
    self_ty: &Type,
    inner: &Ident,
    pre: &TokenStream2,
) -> TokenStream2 {
    quote! {
        #[async_trait::async_trait]
        impl #borsa_core::connector::RecommendationsProvider for #self_ty {
            async fn recommendations(&self, instrument: &#borsa_core::Instrument) -> Result<Vec<#borsa_core::RecommendationRow>, #borsa_core::BorsaError> {
                #pre
                let inner = self.#inner.as_recommendations_provider().ok_or_else(|| #borsa_core::BorsaError::unsupported("recommendations"))?;
                inner.recommendations(instrument).await
            }
        }
    }
}

fn gen_recommendations_summary_impl(
    borsa_core: &Path,
    self_ty: &Type,
    inner: &Ident,
    pre: &TokenStream2,
) -> TokenStream2 {
    quote! {
        #[async_trait::async_trait]
        impl #borsa_core::connector::RecommendationsSummaryProvider for #self_ty {
            async fn recommendations_summary(&self, instrument: &#borsa_core::Instrument) -> Result<#borsa_core::RecommendationSummary, #borsa_core::BorsaError> {
                #pre
                let inner = self.#inner.as_recommendations_summary_provider().ok_or_else(|| #borsa_core::BorsaError::unsupported("recommendations_summary"))?;
                inner.recommendations_summary(instrument).await
            }
        }
    }
}

fn gen_upgrades_impl(
    borsa_core: &Path,
    self_ty: &Type,
    inner: &Ident,
    pre: &TokenStream2,
) -> TokenStream2 {
    quote! {
        #[async_trait::async_trait]
        impl #borsa_core::connector::UpgradesDowngradesProvider for #self_ty {
            async fn upgrades_downgrades(&self, instrument: &#borsa_core::Instrument) -> Result<Vec<#borsa_core::UpgradeDowngradeRow>, #borsa_core::BorsaError> {
                #pre
                let inner = self.#inner.as_upgrades_downgrades_provider().ok_or_else(|| #borsa_core::BorsaError::unsupported("upgrades_downgrades"))?;
                inner.upgrades_downgrades(instrument).await
            }
        }
    }
}

fn gen_price_target_impl(
    borsa_core: &Path,
    self_ty: &Type,
    inner: &Ident,
    pre: &TokenStream2,
) -> TokenStream2 {
    quote! {
        #[async_trait::async_trait]
        impl #borsa_core::connector::AnalystPriceTargetProvider for #self_ty {
            async fn analyst_price_target(&self, instrument: &#borsa_core::Instrument) -> Result<#borsa_core::PriceTarget, #borsa_core::BorsaError> {
                #pre
                let inner = self.#inner.as_analyst_price_target_provider().ok_or_else(|| #borsa_core::BorsaError::unsupported("analyst_price_target"))?;
                inner.analyst_price_target(instrument).await
            }
        }
    }
}

fn gen_major_holders_impl(
    borsa_core: &Path,
    self_ty: &Type,
    inner: &Ident,
    pre: &TokenStream2,
) -> TokenStream2 {
    quote! {
        #[async_trait::async_trait]
        impl #borsa_core::connector::MajorHoldersProvider for #self_ty {
            async fn major_holders(&self, instrument: &#borsa_core::Instrument) -> Result<Vec<#borsa_core::MajorHolder>, #borsa_core::BorsaError> {
                #pre
                let inner = self.#inner.as_major_holders_provider().ok_or_else(|| #borsa_core::BorsaError::unsupported("major_holders"))?;
                inner.major_holders(instrument).await
            }
        }
    }
}

fn gen_institutional_holders_impl(
    borsa_core: &Path,
    self_ty: &Type,
    inner: &Ident,
    pre: &TokenStream2,
) -> TokenStream2 {
    quote! {
        #[async_trait::async_trait]
        impl #borsa_core::connector::InstitutionalHoldersProvider for #self_ty {
            async fn institutional_holders(&self, instrument: &#borsa_core::Instrument) -> Result<Vec<#borsa_core::InstitutionalHolder>, #borsa_core::BorsaError> {
                #pre
                let inner = self.#inner.as_institutional_holders_provider().ok_or_else(|| #borsa_core::BorsaError::unsupported("institutional_holders"))?;
                inner.institutional_holders(instrument).await
            }
        }
    }
}

fn gen_mutual_fund_holders_impl(
    borsa_core: &Path,
    self_ty: &Type,
    inner: &Ident,
    pre: &TokenStream2,
) -> TokenStream2 {
    quote! {
        #[async_trait::async_trait]
        impl #borsa_core::connector::MutualFundHoldersProvider for #self_ty {
            async fn mutual_fund_holders(&self, instrument: &#borsa_core::Instrument) -> Result<Vec<#borsa_core::InstitutionalHolder>, #borsa_core::BorsaError> {
                #pre
                let inner = self.#inner.as_mutual_fund_holders_provider().ok_or_else(|| #borsa_core::BorsaError::unsupported("mutual_fund_holders"))?;
                inner.mutual_fund_holders(instrument).await
            }
        }
    }
}

fn gen_insider_transactions_impl(
    borsa_core: &Path,
    self_ty: &Type,
    inner: &Ident,
    pre: &TokenStream2,
) -> TokenStream2 {
    quote! {
        #[async_trait::async_trait]
        impl #borsa_core::connector::InsiderTransactionsProvider for #self_ty {
            async fn insider_transactions(&self, instrument: &#borsa_core::Instrument) -> Result<Vec<#borsa_core::InsiderTransaction>, #borsa_core::BorsaError> {
                #pre
                let inner = self.#inner.as_insider_transactions_provider().ok_or_else(|| #borsa_core::BorsaError::unsupported("insider_transactions"))?;
                inner.insider_transactions(instrument).await
            }
        }
    }
}

fn gen_insider_roster_impl(
    borsa_core: &Path,
    self_ty: &Type,
    inner: &Ident,
    pre: &TokenStream2,
) -> TokenStream2 {
    quote! {
        #[async_trait::async_trait]
        impl #borsa_core::connector::InsiderRosterHoldersProvider for #self_ty {
            async fn insider_roster_holders(&self, instrument: &#borsa_core::Instrument) -> Result<Vec<#borsa_core::InsiderRosterHolder>, #borsa_core::BorsaError> {
                #pre
                let inner = self.#inner.as_insider_roster_holders_provider().ok_or_else(|| #borsa_core::BorsaError::unsupported("insider_roster_holders"))?;
                inner.insider_roster_holders(instrument).await
            }
        }
    }
}

fn gen_net_share_purchase_impl(
    borsa_core: &Path,
    self_ty: &Type,
    inner: &Ident,
    pre: &TokenStream2,
) -> TokenStream2 {
    quote! {
        #[async_trait::async_trait]
        impl #borsa_core::connector::NetSharePurchaseActivityProvider for #self_ty {
            async fn net_share_purchase_activity(&self, instrument: &#borsa_core::Instrument) -> Result<Option<#borsa_core::NetSharePurchaseActivity>, #borsa_core::BorsaError> {
                #pre
                let inner = self.#inner.as_net_share_purchase_activity_provider().ok_or_else(|| #borsa_core::BorsaError::unsupported("net_share_purchase_activity"))?;
                inner.net_share_purchase_activity(instrument).await
            }
        }
    }
}

fn gen_profile_impl(
    borsa_core: &Path,
    self_ty: &Type,
    inner: &Ident,
    pre: &TokenStream2,
) -> TokenStream2 {
    quote! {
        #[async_trait::async_trait]
        impl #borsa_core::connector::ProfileProvider for #self_ty {
            async fn profile(&self, instrument: &#borsa_core::Instrument) -> Result<#borsa_core::Profile, #borsa_core::BorsaError> {
                #pre
                let inner = self.#inner.as_profile_provider().ok_or_else(|| #borsa_core::BorsaError::unsupported("profile"))?;
                inner.profile(instrument).await
            }
        }
    }
}

fn gen_isin_impl(
    borsa_core: &Path,
    self_ty: &Type,
    inner: &Ident,
    pre: &TokenStream2,
) -> TokenStream2 {
    quote! {
        #[async_trait::async_trait]
        impl #borsa_core::connector::IsinProvider for #self_ty {
            async fn isin(&self, instrument: &#borsa_core::Instrument) -> Result<Option<#borsa_core::Isin>, #borsa_core::BorsaError> {
                #pre
                let inner = self.#inner.as_isin_provider().ok_or_else(|| #borsa_core::BorsaError::unsupported("isin"))?;
                inner.isin(instrument).await
            }
        }
    }
}

fn gen_search_impl(
    borsa_core: &Path,
    self_ty: &Type,
    inner: &Ident,
    pre: &TokenStream2,
) -> TokenStream2 {
    quote! {
        #[async_trait::async_trait]
        impl #borsa_core::connector::SearchProvider for #self_ty {
            async fn search(&self, req: #borsa_core::SearchRequest) -> Result<#borsa_core::SearchResponse, #borsa_core::BorsaError> {
                #pre
                let inner = self.#inner.as_search_provider().ok_or_else(|| #borsa_core::BorsaError::unsupported("search"))?;
                inner.search(req).await
            }
        }
    }
}

fn gen_esg_impl(
    borsa_core: &Path,
    self_ty: &Type,
    inner: &Ident,
    pre: &TokenStream2,
) -> TokenStream2 {
    quote! {
        #[async_trait::async_trait]
        impl #borsa_core::connector::EsgProvider for #self_ty {
            async fn sustainability(&self, instrument: &#borsa_core::Instrument) -> Result<#borsa_core::EsgScores, #borsa_core::BorsaError> {
                #pre
                let inner = self.#inner.as_esg_provider().ok_or_else(|| #borsa_core::BorsaError::unsupported("sustainability"))?;
                inner.sustainability(instrument).await
            }
        }
    }
}

fn gen_news_impl(
    borsa_core: &Path,
    self_ty: &Type,
    inner: &Ident,
    pre: &TokenStream2,
) -> TokenStream2 {
    quote! {
        #[async_trait::async_trait]
        impl #borsa_core::connector::NewsProvider for #self_ty {
            async fn news(&self, instrument: &#borsa_core::Instrument, req: #borsa_core::NewsRequest) -> Result<Vec<#borsa_core::types::NewsArticle>, #borsa_core::BorsaError> {
                #pre
                let inner = self.#inner.as_news_provider().ok_or_else(|| #borsa_core::BorsaError::unsupported("news"))?;
                inner.news(instrument, req).await
            }
        }
    }
}

fn gen_options_expirations_impl(
    borsa_core: &Path,
    self_ty: &Type,
    inner: &Ident,
    pre: &TokenStream2,
) -> TokenStream2 {
    quote! {
        #[async_trait::async_trait]
        impl #borsa_core::connector::OptionsExpirationsProvider for #self_ty {
            async fn options_expirations(&self, instrument: &#borsa_core::Instrument) -> Result<Vec<i64>, #borsa_core::BorsaError> {
                #pre
                let inner = self.#inner.as_options_expirations_provider().ok_or_else(|| #borsa_core::BorsaError::unsupported("options_expirations"))?;
                inner.options_expirations(instrument).await
            }
        }
    }
}

fn gen_option_chain_impl(
    borsa_core: &Path,
    self_ty: &Type,
    inner: &Ident,
    pre: &TokenStream2,
) -> TokenStream2 {
    quote! {
        #[async_trait::async_trait]
        impl #borsa_core::connector::OptionChainProvider for #self_ty {
            async fn option_chain(&self, instrument: &#borsa_core::Instrument, date: Option<i64>) -> Result<#borsa_core::OptionChain, #borsa_core::BorsaError> {
                #pre
                let inner = self.#inner.as_option_chain_provider().ok_or_else(|| #borsa_core::BorsaError::unsupported("option_chain"))?;
                inner.option_chain(instrument, date).await
            }
        }
    }
}

fn gen_stream_impl(
    borsa_core: &Path,
    self_ty: &Type,
    inner: &Ident,
    pre: &TokenStream2,
) -> TokenStream2 {
    quote! {
        #[async_trait::async_trait]
        impl #borsa_core::connector::StreamProvider for #self_ty {
            async fn stream_quotes(&self, instruments: &[#borsa_core::Instrument]) -> Result<(#borsa_core::stream::StreamHandle, tokio::sync::mpsc::Receiver<#borsa_core::QuoteUpdate>), #borsa_core::BorsaError> {
                #pre
                let inner = self.#inner.as_stream_provider().ok_or_else(|| #borsa_core::BorsaError::unsupported("stream_quotes"))?;
                inner.stream_quotes(instruments).await
            }
        }
    }
}

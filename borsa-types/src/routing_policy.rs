//! Centralized routing policy for provider and exchange ordering.
//!
//! This module defines a composable policy and an ergonomic builder to steer
//! connector and exchange preferences at global, per-kind and per-symbol
//! scopes. Routers consume the stable sort-keys exposed here to avoid
//! duplicating ordering logic across call sites.
//!
//! Provider selection and exchange preferences are related but distinct:
//! - Provider rules: select and order which connectors are eligible to be called.
//!   Rules are matched against a [`RoutingContext`]. When multiple rules match, the
//!   one with the highest [specificity](Selector::specificity) wins (i.e., the
//!   rule with more populated selector fields). Ties are broken by preferring
//!   rules that target a symbol, then a kind, then an exchange; if a tie remains,
//!   the rule defined last wins. A rule can be marked `strict` to exclude any
//!   provider that is not explicitly listed by that rule. A global rule applies
//!   when no more-specific rule matches.
//! - Exchange preferences: provide an ordering for exchanges and are currently
//!   used by search de-duplication to pick the best result per symbol. Exchange
//!   preferences resolve by scope using Symbol > Kind > Global precedence, which
//!   differs from provider rule tie-breaking.
//!
//! Notes:
//! - The builder validates connector keys during [`borsa`]'s build step; unknown
//!   connector names cause an error. See
//!   [`BorsaBuilder::build`](https://docs.rs/borsa/latest/borsa/struct.BorsaBuilder.html#method.build).
//! - Unlisted providers will still be eligible when a matching rule is not
//!   `strict`; they will be placed after listed ones, preserving registration
//!   order.

use std::collections::{HashMap, HashSet};

use crate::connector::ConnectorKey;
use paft::domain::{AssetKind, Exchange};
use serde::de::{SeqAccess, Visitor};
use serde::ser::SerializeSeq;
use serde::{Deserialize, Serialize};

type Specificity = (u8, u8, u8, u8);
type ProviderMatch<'a> = (&'a RankedList<ConnectorKey>, bool, Specificity, usize);

/// Scope at which a preference applies. Precedence is Symbol > Kind > Global.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScopeKey {
    /// Global scope, used when no symbol- or kind-specific preference exists.
    Global,
    /// Preference bound to a specific asset kind.
    Kind(AssetKind),
    /// Preference bound to a specific symbol string.
    Symbol(String),
}

/// Ranked list of values with cached index positions for stable sort keys.
#[derive(Debug, Clone)]
pub(crate) struct RankedList<T> {
    values: Vec<T>,
    ranks: HashMap<T, usize>,
}

impl<T> RankedList<T>
where
    T: Clone + Eq + std::hash::Hash,
{
    fn new(list: &[T]) -> Self {
        let mut values: Vec<T> = Vec::new();
        let mut seen: HashSet<T> = HashSet::new();
        for value in list.iter().cloned() {
            if seen.insert(value.clone()) {
                values.push(value);
            }
        }

        let mut ranks: HashMap<T, usize> = HashMap::with_capacity(values.len());
        for (idx, value) in values.iter().cloned().enumerate() {
            ranks.insert(value, idx);
        }

        Self { values, ranks }
    }

    fn values(&self) -> &[T] {
        &self.values
    }

    const fn ranks(&self) -> &HashMap<T, usize> {
        &self.ranks
    }
}

impl<T> Serialize for RankedList<T>
where
    T: Serialize + Clone + Eq + std::hash::Hash,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.values.len()))?;
        for v in &self.values {
            seq.serialize_element(v)?;
        }
        seq.end()
    }
}

impl<'de, T> Deserialize<'de> for RankedList<T>
where
    T: Deserialize<'de> + Clone + Eq + std::hash::Hash,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct RLVisitor<TV> {
            _m: std::marker::PhantomData<TV>,
        }
        impl<'de, TV> Visitor<'de> for RLVisitor<TV>
        where
            TV: Deserialize<'de> + Clone + Eq + std::hash::Hash,
        {
            type Value = RankedList<TV>;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "a sequence of values")
            }
            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut vals: Vec<TV> = Vec::new();
                while let Some(elem) = seq.next_element::<TV>()? {
                    vals.push(elem);
                }
                Ok(RankedList::new(&vals))
            }
        }
        deserializer.deserialize_seq(RLVisitor {
            _m: std::marker::PhantomData,
        })
    }
}

/// Ordered preference list keyed by a [`ScopeKey`].
///
/// Values are de-duplicated while preserving the first occurrence order.
#[derive(Debug, Clone)]
pub struct Preference<T> {
    global: Option<RankedList<T>>,
    by_kind: HashMap<AssetKind, RankedList<T>>,
    by_symbol: HashMap<String, RankedList<T>>,
}

impl<T> Default for Preference<T>
where
    T: Clone + Eq + std::hash::Hash,
{
    fn default() -> Self {
        Self {
            global: None,
            by_kind: HashMap::new(),
            by_symbol: HashMap::new(),
        }
    }
}

impl<T> Preference<T>
where
    T: Clone + Eq + std::hash::Hash,
{
    /// Set the ordered list for `scope`, keeping only the first occurrence of
    /// each element and preserving order.
    pub fn set(&mut self, scope: ScopeKey, list: &[T]) {
        let ranked = RankedList::new(list);
        match scope {
            ScopeKey::Global => {
                self.global = Some(ranked);
            }
            ScopeKey::Kind(kind) => {
                self.by_kind.insert(kind, ranked);
            }
            ScopeKey::Symbol(symbol) => {
                self.by_symbol.insert(symbol, ranked);
            }
        }
    }

    /// Remove all configured preferences.
    pub fn clear(&mut self) {
        self.global = None;
        self.by_kind.clear();
        self.by_symbol.clear();
    }

    /// Iterate over configured scopes in unspecified order.
    #[must_use]
    pub fn scopes(&self) -> PreferenceScopeIter<'_, T> {
        PreferenceScopeIter {
            global: self.global.as_ref(),
            yielded_global: false,
            by_kind: self.by_kind.iter(),
            by_symbol: self.by_symbol.iter(),
        }
    }

    /// Resolve the highest-precedence list for `(symbol, kind)` following the
    /// Symbol > Kind > Global ordering. Returns `None` if no preference exists.
    #[must_use]
    pub fn resolve<'a>(&'a self, symbol: Option<&str>, kind: Option<AssetKind>) -> Option<&'a [T]> {
        if let Some(sym) = symbol
            && let Some(list) = self.by_symbol.get(sym)
        {
            return Some(list.values());
        }
        if let Some(k) = kind
            && let Some(list) = self.by_kind.get(&k)
        {
            return Some(list.values());
        }
        self.global.as_ref().map(RankedList::values)
    }

    /// Resolve the highest-precedence rank map for `(symbol, kind)`, mirroring
    /// [`resolve`] but returning the cached rank table.
    #[must_use]
    pub fn resolve_rank_map(
        &self,
        symbol: Option<&str>,
        kind: Option<AssetKind>,
    ) -> Option<&HashMap<T, usize>> {
        if let Some(sym) = symbol
            && let Some(list) = self.by_symbol.get(sym)
        {
            return Some(list.ranks());
        }
        if let Some(k) = kind
            && let Some(list) = self.by_kind.get(&k)
        {
            return Some(list.ranks());
        }
        self.global.as_ref().map(RankedList::ranks)
    }
}

impl<T> Serialize for Preference<T>
where
    T: Serialize + Clone + Eq + std::hash::Hash,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut st = serializer.serialize_struct("Preference", 3)?;
        st.serialize_field("global", &self.global)?;
        st.serialize_field("by_kind", &self.by_kind)?;
        st.serialize_field("by_symbol", &self.by_symbol)?;
        st.end()
    }
}

impl<'de, T> Deserialize<'de> for Preference<T>
where
    T: Deserialize<'de> + Clone + Eq + std::hash::Hash,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(bound(deserialize = "T: Deserialize<'de> + Clone + Eq + std::hash::Hash"))]
        struct PrefSerde<T> {
            global: Option<RankedList<T>>,
            by_kind: Option<HashMap<AssetKind, RankedList<T>>>,
            by_symbol: Option<HashMap<String, RankedList<T>>>,
        }
        let tmp = PrefSerde::deserialize(deserializer)?;
        Ok(Self {
            global: tmp.global,
            by_kind: tmp.by_kind.unwrap_or_default(),
            by_symbol: tmp.by_symbol.unwrap_or_default(),
        })
    }
}

/// Iterator over configured preference scopes.
pub struct PreferenceScopeIter<'a, T> {
    global: Option<&'a RankedList<T>>,
    yielded_global: bool,
    by_kind: std::collections::hash_map::Iter<'a, AssetKind, RankedList<T>>,
    by_symbol: std::collections::hash_map::Iter<'a, String, RankedList<T>>,
}

impl<'a, T> Iterator for PreferenceScopeIter<'a, T>
where
    T: Clone + Eq + std::hash::Hash,
{
    type Item = PreferenceScope<'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        if !self.yielded_global {
            self.yielded_global = true;
            if let Some(global) = self.global {
                return Some(PreferenceScope::Global(PreferenceValues::new(global)));
            }
        }

        if let Some((kind, list)) = self.by_kind.next() {
            return Some(PreferenceScope::Kind(*kind, PreferenceValues::new(list)));
        }

        if let Some((symbol, list)) = self.by_symbol.next() {
            return Some(PreferenceScope::Symbol(
                symbol.as_str(),
                PreferenceValues::new(list),
            ));
        }

        None
    }
}

/// Borrowed scope entry yielded by [`Preference::scopes`].
#[derive(Clone, Copy)]
pub enum PreferenceScope<'a, T> {
    /// Global preference values, if configured.
    Global(PreferenceValues<'a, T>),
    /// Preference values for a specific asset kind.
    Kind(AssetKind, PreferenceValues<'a, T>),
    /// Preference values for a specific symbol.
    Symbol(&'a str, PreferenceValues<'a, T>),
}

impl<'a, T> PreferenceScope<'a, T>
where
    T: Clone + Eq + std::hash::Hash,
{
    /// Convert the borrowed scope into an owned [`ScopeKey`].
    #[must_use]
    pub fn to_scope_key(&self) -> ScopeKey {
        match self {
            PreferenceScope::Global(_) => ScopeKey::Global,
            PreferenceScope::Kind(kind, _) => ScopeKey::Kind(*kind),
            PreferenceScope::Symbol(symbol, _) => ScopeKey::Symbol((*symbol).to_string()),
        }
    }

    /// Borrow the ordered values associated with the scope.
    #[must_use]
    pub const fn values(&self) -> &'a [T] {
        match self {
            PreferenceScope::Global(values)
            | PreferenceScope::Kind(_, values)
            | PreferenceScope::Symbol(_, values) => values.values(),
        }
    }

    /// Borrow the cached rank map associated with the scope.
    #[must_use]
    pub const fn rank_map(&self) -> &'a HashMap<T, usize> {
        match self {
            PreferenceScope::Global(values)
            | PreferenceScope::Kind(_, values)
            | PreferenceScope::Symbol(_, values) => values.rank_map(),
        }
    }
}

/// Borrowed preference data (values + cached ranks) for a specific scope.
#[derive(Clone, Copy)]
pub struct PreferenceValues<'a, T> {
    values: &'a [T],
    ranks: &'a HashMap<T, usize>,
}

impl<'a, T> PreferenceValues<'a, T>
where
    T: Clone + Eq + std::hash::Hash,
{
    fn new(list: &'a RankedList<T>) -> Self {
        Self {
            values: list.values(),
            ranks: list.ranks(),
        }
    }

    /// Borrow the ordered values associated with this scope.
    #[must_use]
    pub const fn values(&self) -> &'a [T] {
        self.values
    }

    /// Borrow the cached rank map associated with this scope.
    #[must_use]
    pub const fn rank_map(&self) -> &'a HashMap<T, usize> {
        self.ranks
    }
}

/// Generic selector identifying when a provider rule applies.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Selector {
    /// Optional symbol constraint for a rule. When set, the rule applies only to this symbol.
    pub symbol: Option<String>,
    /// Optional asset kind constraint. When set, the rule applies only to this kind.
    pub kind: Option<AssetKind>,
    /// Optional exchange constraint. When set, the rule applies only to this exchange.
    pub exchange: Option<Exchange>,
}

impl Selector {
    /// Compute precedence bits for tie-breaking between selectors.
    #[must_use]
    pub const fn specificity_bits(&self) -> (u8, u8, u8) {
        (
            self.symbol.is_some() as u8,
            self.kind.is_some() as u8,
            self.exchange.is_some() as u8,
        )
    }
}

/// A single provider rule with its selector, ordered connector list and strict flag.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderRule {
    /// The selector describing when this rule applies.
    pub selector: Selector,
    pub(crate) list: RankedList<ConnectorKey>,
    /// When true, only the providers listed by this rule are eligible. When false, providers
    /// not explicitly listed remain eligible after listed ones, preserving registration order.
    pub strict: bool,
}

/// Provider policy composed of ordered matching rules and an optional global rule.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderPolicy {
    pub(crate) rules: Vec<ProviderRule>,
    pub(crate) global: Option<(RankedList<ConnectorKey>, bool)>,
}

impl ProviderPolicy {
    /// Select the best-matching rule for the provided context.
    ///
    /// Tie-breaking: higher [`Selector::specificity`] wins; if equal specificity,
    /// the rule defined later wins. Returns the list and its `strict` flag.
    fn best_rule<'a>(
        &'a self,
        ctx: &RoutingContext,
    ) -> Option<(&'a RankedList<ConnectorKey>, bool)> {
        let mut best: Option<ProviderMatch<'_>> = None;
        for (idx, r) in self.rules.iter().enumerate() {
            let s = &r.selector;
            if s.symbol.is_some() && s.symbol.as_deref() != ctx.symbol {
                continue;
            }
            if s.kind.is_some() && s.kind != ctx.kind {
                continue;
            }
            if s.exchange.is_some() && s.exchange != ctx.exchange {
                continue;
            }
            let (sb, kb, eb) = s.specificity_bits();
            let count = sb + kb + eb;
            let spec: Specificity = (count, sb, kb, eb);
            match best {
                None => best = Some((&r.list, r.strict, spec, idx)),
                Some((_, _, bspec, bidx)) => {
                    if spec > bspec || (spec == bspec && idx > bidx) {
                        best = Some((&r.list, r.strict, spec, idx));
                    }
                }
            }
        }
        best.map(|(list, strict, _, _)| (list, strict))
    }

    /// Returns Some((rank, strict)) for a provider key if eligible in this context, otherwise None.
    ///
    /// Semantics:
    /// - If a matching rule exists and contains `key`, the returned rank is its
    ///   position in that rule's list.
    /// - If a matching rule exists but does not include `key` and the rule is
    ///   `strict`, the provider is ineligible (returns `None`).
    /// - If no matching rule exists, the global rule is considered next with the
    ///   same semantics.
    /// - If neither matches, the provider is eligible with `usize::MAX` rank (i.e.,
    ///   after any explicitly listed providers).
    #[must_use]
    pub fn provider_rank(&self, ctx: &RoutingContext, key: &ConnectorKey) -> Option<(usize, bool)> {
        if let Some((list, strict)) = self.best_rule(ctx) {
            if let Some(rank) = list.ranks().get(key).copied() {
                return Some((rank, strict));
            }
            return if strict {
                None
            } else {
                Some((usize::MAX, false))
            };
        }
        if let Some((global, strict)) = &self.global {
            if let Some(rank) = global.ranks().get(key).copied() {
                return Some((rank, *strict));
            }
            return if *strict {
                None
            } else {
                Some((usize::MAX, false))
            };
        }
        Some((usize::MAX, false))
    }

    /// Set or replace the global provider ordering and strictness.
    pub fn set_global(&mut self, list: &[ConnectorKey], strict: bool) {
        self.global = Some((RankedList::new(list), strict));
    }

    /// Append a provider rule; later rules of equal specificity override earlier ones.
    pub fn add_rule(&mut self, selector: Selector, list: &[ConnectorKey], strict: bool) {
        self.rules.push(ProviderRule {
            selector,
            list: RankedList::new(list),
            strict,
        });
    }

    /// Iterate rules (for builder validation).
    pub fn iter_rules(&self) -> impl Iterator<Item = &ProviderRule> {
        self.rules.iter()
    }

    /// Normalize provider lists against a set of known connector names and collect unknowns.
    ///
    /// - Drops duplicate connectors while preserving first occurrence order.
    /// - Filters out unknown connector keys; returns them grouped by selector.
    /// - Used by `borsa` during build to reject policies that reference unknown
    ///   connectors. Callers typically surface the returned list as an error.
    pub fn normalize_and_collect_unknown(
        &mut self,
        known: &std::collections::HashSet<&'static str>,
    ) -> Vec<(Selector, Vec<String>)> {
        let mut unknown: Vec<(Selector, Vec<String>)> = Vec::new();

        if let Some((global, _strict)) = &mut self.global {
            let mut filtered: Vec<ConnectorKey> = Vec::new();
            let mut seen: HashSet<String> = HashSet::new();
            let mut missing: Vec<String> = Vec::new();
            for key in global.values.iter().cloned() {
                let name = key.as_str();
                if known.contains(name) {
                    if seen.insert(name.to_string()) {
                        filtered.push(key);
                    }
                } else {
                    missing.push(name.to_string());
                }
            }
            *global = RankedList::new(&filtered);
            if !missing.is_empty() {
                unknown.push((
                    Selector {
                        symbol: None,
                        kind: None,
                        exchange: None,
                    },
                    missing,
                ));
            }
        }

        for rule in &mut self.rules {
            let mut filtered: Vec<ConnectorKey> = Vec::new();
            let mut seen: HashSet<String> = HashSet::new();
            let mut missing: Vec<String> = Vec::new();
            for key in rule.list.values.iter().cloned() {
                let name = key.as_str();
                if known.contains(name) {
                    if seen.insert(name.to_string()) {
                        filtered.push(key);
                    }
                } else {
                    missing.push(name.to_string());
                }
            }
            rule.list = RankedList::new(&filtered);
            if !missing.is_empty() {
                unknown.push((rule.selector.clone(), missing));
            }
        }

        unknown
    }
}

/// Routing policy aggregating provider and exchange preferences.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RoutingPolicy {
    /// Provider routing policy (rules + global).
    pub providers: ProviderPolicy,
    /// Exchange ordering preferences (used by search de-duplication).
    pub exchanges: Preference<Exchange>,
}

/// Builder for a [`RoutingPolicy`]. Later calls for the same scope overwrite
/// earlier ones.
#[derive(Debug, Clone, Default)]
pub struct RoutingPolicyBuilder {
    policy: RoutingPolicy,
}

impl RoutingPolicyBuilder {
    /// Create a new empty routing policy builder.
    #[must_use]
    pub fn new() -> Self {
        Self {
            policy: RoutingPolicy::default(),
        }
    }

    /// Set a global provider ordering (fallback allowed).
    ///
    /// Providers not listed remain eligible after the listed ones unless a
    /// more specific strict rule applies in a given context.
    #[must_use]
    pub fn providers_global(mut self, list: &[ConnectorKey]) -> Self {
        self.policy.providers.set_global(list, false);
        self
    }

    /// Set a global provider ordering (no fallback to other providers).
    ///
    /// When strict, only providers explicitly listed are eligible in contexts
    /// where no more specific rule matches.
    #[must_use]
    pub fn providers_global_strict(mut self, list: &[ConnectorKey]) -> Self {
        self.policy.providers.set_global(list, true);
        self
    }

    /// Set provider ordering for a specific kind (fallback allowed).
    ///
    /// If multiple rules match a context, higher specificity wins; on tie,
    /// the rule added later takes precedence.
    #[must_use]
    pub fn providers_for_kind(mut self, kind: AssetKind, list: &[ConnectorKey]) -> Self {
        let selector = Selector {
            symbol: None,
            kind: Some(kind),
            exchange: None,
        };
        self.policy.providers.add_rule(selector, list, false);
        self
    }

    /// Set provider ordering for a specific symbol (fallback allowed).
    ///
    /// Symbol rules are more specific than kind or exchange-only rules and
    /// therefore take precedence when they match the current context.
    #[must_use]
    pub fn providers_for_symbol(mut self, symbol: &str, list: &[ConnectorKey]) -> Self {
        let selector = Selector {
            symbol: Some(symbol.to_string()),
            kind: None,
            exchange: None,
        };
        self.policy.providers.add_rule(selector, list, false);
        self
    }

    /// Set provider ordering for a specific exchange (fallback allowed).
    ///
    /// This rule matches contexts where the exchange is known. It is distinct
    /// from exchange preferences (which are used for search dedup, not provider
    /// eligibility).
    #[must_use]
    pub fn providers_for_exchange(mut self, exchange: Exchange, list: &[ConnectorKey]) -> Self {
        let selector = Selector {
            symbol: None,
            kind: None,
            exchange: Some(exchange),
        };
        self.policy.providers.add_rule(selector, list, false);
        self
    }

    /// Add a fully-composable provider rule with a strict flag.
    ///
    /// Use this when you need to combine constraints (e.g., symbol+kind) or
    /// set a strict rule that disables fallback to unlisted providers.
    #[must_use]
    pub fn providers_rule(
        mut self,
        selector: Selector,
        list: &[ConnectorKey],
        strict: bool,
    ) -> Self {
        self.policy.providers.add_rule(selector, list, strict);
        self
    }

    /// Set a global exchange ordering.
    ///
    /// Exchange preferences affect search result de-duplication only. They do
    /// not change which providers are eligible; use provider rules for that.
    #[must_use]
    pub fn exchanges_global(mut self, list: &[Exchange]) -> Self {
        self.policy.exchanges.set(ScopeKey::Global, list);
        self
    }

    /// Set exchange ordering for a specific kind.
    #[must_use]
    pub fn exchanges_for_kind(mut self, kind: AssetKind, list: &[Exchange]) -> Self {
        self.policy.exchanges.set(ScopeKey::Kind(kind), list);
        self
    }

    /// Set exchange ordering for a specific symbol.
    #[must_use]
    pub fn exchanges_for_symbol(mut self, symbol: &str, list: &[Exchange]) -> Self {
        self.policy
            .exchanges
            .set(ScopeKey::Symbol(symbol.to_string()), list);
        self
    }

    /// Finalize and return the built policy.
    #[must_use]
    pub fn build(self) -> RoutingPolicy {
        self.policy
    }
}

/// Routing context used when evaluating precedence and computing sort keys.
#[derive(Debug, Clone)]
pub struct RoutingContext<'a> {
    /// Optional symbol under consideration.
    pub symbol: Option<&'a str>,
    /// Optional asset kind under consideration.
    pub kind: Option<AssetKind>,
    /// Optional exchange under consideration.
    pub exchange: Option<Exchange>,
}

impl<'a> RoutingContext<'a> {
    /// Construct a new context from optional `symbol` and `kind`.
    #[must_use]
    pub const fn new(
        symbol: Option<&'a str>,
        kind: Option<AssetKind>,
        exchange: Option<Exchange>,
    ) -> Self {
        Self {
            symbol,
            kind,
            exchange,
        }
    }
}

impl RoutingPolicy {
    /// Compute a stable sort key for provider ordering using the provider policy.
    ///
    /// Returns (rank, `orig_idx`) where unknown providers rank after known ones.
    #[must_use]
    pub fn provider_sort_key(
        &self,
        ctx: &RoutingContext,
        key: &ConnectorKey,
        orig_idx: usize,
    ) -> (usize, usize) {
        let (rank, _strict) = self
            .providers
            .provider_rank(ctx, key)
            .unwrap_or((usize::MAX, false));
        (rank, orig_idx)
    }

    /// Compute a stable sort key for exchange-based de-duplication.
    ///
    /// Sorts by preference rank (lower wins; `usize::MAX` for unknown), then a
    /// penalty for `None` exchanges (unknowns last), a reserved slot for future
    /// tie-breakers, and the original index as a final tie-breaker.
    #[must_use]
    pub fn exchange_sort_key(
        &self,
        ctx: &RoutingContext,
        ex: Option<&Exchange>,
        orig_idx: usize,
    ) -> (usize, usize, usize, usize) {
        let none_penalty = if ex.is_some() { 0 } else { usize::MAX };
        let rank_map = self.exchanges.resolve_rank_map(ctx.symbol, ctx.kind);
        let rank = ex
            .and_then(|e| rank_map.and_then(|m| m.get(e)).copied())
            .unwrap_or(usize::MAX);
        (rank, none_penalty, usize::MAX, orig_idx)
    }
}

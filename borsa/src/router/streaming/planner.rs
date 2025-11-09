use std::collections::HashSet;
use std::sync::Arc;

use crate::Borsa;
use borsa_core::{
    AssetKind, BorsaConnector, BorsaError, Capability, Exchange, Instrument, RoutingContext, Symbol,
};

/// Providers eligible for this (kind, exchange) group sorted by score and registration order
pub struct EligibleStreamProviders {
    pub providers: Vec<Arc<dyn BorsaConnector>>,
    /// Allowed symbols per provider, aligned with `providers`
    pub provider_symbols: Vec<HashSet<Symbol>>,
    /// Union of all allowed symbols across providers
    pub union_symbols: HashSet<Symbol>,
}

type StreamProviderScore = (usize, usize, Arc<dyn BorsaConnector>, HashSet<Symbol>);

impl Borsa {
    fn check_no_scored_stream_providers(
        &self,
        kind: AssetKind,
        exchange: Option<&Exchange>,
        instruments: &[Instrument],
    ) -> Result<EligibleStreamProviders, BorsaError> {
        // If we have streaming-capable providers for this kind, check whether strict routing
        // rules filtered out every requested symbol. Otherwise surface the original
        // Unsupported error.
        let candidates: Vec<&Arc<dyn BorsaConnector>> = self
            .connectors
            .iter()
            .filter(|c| c.as_stream_provider().is_some() && c.supports_kind(kind))
            .collect();

        if !candidates.is_empty() {
            let mut strict_rejected: Vec<Symbol> = Vec::new();
            for inst in instruments {
                let sym_opt = match inst.id() {
                    borsa_core::IdentifierScheme::Security(sec) => Some(&sec.symbol),
                    borsa_core::IdentifierScheme::Prediction(_) => None,
                };
                let ex_opt = match inst.id() {
                    borsa_core::IdentifierScheme::Security(sec) => sec.exchange.clone(),
                    borsa_core::IdentifierScheme::Prediction(_) => None,
                };
                let ctx =
                    RoutingContext::new(sym_opt, Some(kind), ex_opt.or_else(|| exchange.cloned()));
                let any_allowed = candidates.iter().any(|c| {
                    self.cfg
                        .routing_policy
                        .providers
                        .provider_rank(&ctx, &c.key())
                        .is_some()
                });
                if !any_allowed && let Some(sym) = sym_opt {
                    strict_rejected.push(sym.clone());
                }
            }
            if !strict_rejected.is_empty() {
                strict_rejected.sort();
                strict_rejected.dedup();
                return Err(BorsaError::StrictSymbolsRejected {
                    rejected: strict_rejected,
                });
            }
        }

        Err(BorsaError::unsupported(
            Capability::StreamQuotes.to_string(),
        ))
    }
    pub(crate) fn eligible_stream_providers_for_context(
        &self,
        kind: AssetKind,
        exchange: Option<&Exchange>,
        instruments: &[Instrument],
    ) -> Result<EligibleStreamProviders, BorsaError> {
        // Score all connectors by the minimum per-symbol rank across the requested instruments,
        // then sort by (min_rank, registration_index). Collect allowed symbols in the process.
        let mut scored: Vec<StreamProviderScore> = Vec::new();

        for (orig_idx, connector) in self.connectors.iter().cloned().enumerate() {
            if connector.as_stream_provider().is_none() {
                continue;
            }
            if !connector.supports_kind(kind) {
                continue;
            }

            let mut allowed_syms: HashSet<Symbol> = HashSet::new();
            let mut min_rank: usize = usize::MAX;
            for inst in instruments {
                let sym_opt = match inst.id() {
                    borsa_core::IdentifierScheme::Security(sec) => Some(&sec.symbol),
                    borsa_core::IdentifierScheme::Prediction(_) => None,
                };
                let ex_opt = match inst.id() {
                    borsa_core::IdentifierScheme::Security(sec) => sec.exchange.clone(),
                    borsa_core::IdentifierScheme::Prediction(_) => None,
                };
                let ctx =
                    RoutingContext::new(sym_opt, Some(kind), ex_opt.or_else(|| exchange.cloned()));
                if let Some((rank, _strict)) = self
                    .cfg
                    .routing_policy
                    .providers
                    .provider_rank(&ctx, &connector.key())
                {
                    if let Some(sym) = sym_opt {
                        allowed_syms.insert(sym.clone());
                    }
                    if rank < min_rank {
                        min_rank = rank;
                    }
                }
            }

            if !allowed_syms.is_empty() {
                scored.push((min_rank, orig_idx, connector, allowed_syms));
            }
        }

        if scored.is_empty() {
            return self.check_no_scored_stream_providers(kind, exchange, instruments);
        }

        scored.sort_by_key(|(min_rank, orig_idx, _, _)| (*min_rank, *orig_idx));

        let mut providers: Vec<Arc<dyn BorsaConnector>> = Vec::new();
        let mut provider_symbols: Vec<HashSet<Symbol>> = Vec::new();
        let mut union_symbols: HashSet<Symbol> = HashSet::new();

        for (_, _, c, syms) in scored {
            union_symbols.extend(syms.iter().cloned());
            providers.push(c);
            provider_symbols.push(syms);
        }

        Ok(EligibleStreamProviders {
            providers,
            provider_symbols,
            union_symbols,
        })
    }
}

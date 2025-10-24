use crate::Borsa;
use crate::borsa_router_method;
use borsa_core::{
    Capability, Instrument, PriceTarget, RecommendationRow, RecommendationSummary,
    UpgradeDowngradeRow,
};

impl Borsa {
    borsa_router_method! {
        /// Fetch detailed analyst recommendation rows for an instrument.
        ///
        /// Behavior: routes according to provider priorities and fetch strategy; may
        /// return an empty vector when a provider is reachable but has no data.
        method: recommendations(inst: &Instrument) -> Vec<RecommendationRow>,
        provider: RecommendationsProvider,
        accessor: as_recommendations_provider,
        capability: Capability::Recommendations,
        not_found: "analysis",
        call: recommendations(inst)
    }

    borsa_router_method! {
        /// Fetch the summarized recommendation snapshot for an instrument.
        ///
        /// Trade-offs: a compact summary suitable for dashboards; for full detail,
        /// use `recommendations`.
        method: recommendations_summary(inst: &Instrument) -> RecommendationSummary,
        provider: RecommendationsSummaryProvider,
        accessor: as_recommendations_summary_provider,
        capability: Capability::RecommendationsSummary,
        not_found: "analysis",
        call: recommendations_summary(inst)
    }

    borsa_router_method! {
        /// Fetch broker upgrade/downgrade history for an instrument.
        ///
        /// Behavior: time-ordered events when available; gaps and provider-specific
        /// classifications are passed through without normalization.
        method: upgrades_downgrades(inst: &Instrument) -> Vec<UpgradeDowngradeRow>,
        provider: UpgradesDowngradesProvider,
        accessor: as_upgrades_downgrades_provider,
        capability: Capability::UpgradesDowngrades,
        not_found: "analysis",
        call: upgrades_downgrades(inst)
    }

    borsa_router_method! {
        /// Fetch the analyst price target summary for an instrument.
        ///
        /// Notes: number of analysts and distribution depend on the provider's
        /// coverage; values may lag real-time broker changes.
        method: analyst_price_target(inst: &Instrument) -> PriceTarget,
        provider: AnalystPriceTargetProvider,
        accessor: as_analyst_price_target_provider,
        capability: Capability::AnalystPriceTarget,
        not_found: "analysis",
        call: analyst_price_target(inst)
    }
}

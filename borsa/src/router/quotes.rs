use borsa_core::{BorsaError, Quote};
// QuoteProvider trait is used via returned trait objects; no direct import needed

use crate::Borsa;
use crate::borsa_router_method;

impl Borsa {
    borsa_router_method! {
        /// Fetch a point-in-time quote for a single instrument.
        ///
        /// Behavior and trade-offs:
        /// - Honors the builder's `FetchStrategy`: `PriorityWithFallback` applies the
        ///   per-provider timeout and aggregates errors; `Latency` races providers and
        ///   returns the first success (lower latency, higher request fanout).
        /// - `NotFound` from any attempted provider maps to a `NotFound` outcome when
        ///   using fallback; with latency mode, the first success wins and failures are
        ///   aggregated only if all attempts fail.
        method: quote(inst: &borsa_core::Instrument) -> borsa_core::Quote,
        provider: QuoteProvider,
        accessor: as_quote_provider,
        capability: "quote",
        not_found: "quote",
        call: quote(inst)
    }

    /// Fetch quotes for multiple instruments.
    ///
    /// Behavior and trade-offs:
    /// - Executes single-quote requests concurrently and aggregates outcomes.
    /// - Returns `(successful_quotes, failures)` where `failures` contains per-symbol
    ///   errors (including `NotFound`). This allows partial success without failing the
    ///   entire batch.
    /// - Overall `Err` is returned only if joining tasks fails or a systemic error
    ///   occurs before per-symbol routing.
    ///
    /// # Errors
    /// Returns an error only if joining tasks fails before per-symbol routing.
    pub async fn quotes(
        &self,
        insts: &[borsa_core::Instrument],
    ) -> Result<(Vec<Quote>, Vec<(borsa_core::Instrument, BorsaError)>), BorsaError> {
        if insts.is_empty() {
            return Ok((vec![], vec![]));
        }

        let tasks = insts.iter().map(|inst| {
            let borsa = self;
            let inst_clone = inst.clone();
            async move {
                let res = borsa.quote(&inst_clone).await;
                (inst_clone, res)
            }
        });

        let results = futures::future::join_all(tasks).await;

        let mut ok_quotes: Vec<Quote> = Vec::new();
        let mut failures: Vec<(borsa_core::Instrument, BorsaError)> = Vec::new();
        for (inst, res) in results {
            match res {
                Ok(q) => ok_quotes.push(q),
                Err(e) => failures.push((inst, e)),
            }
        }

        Ok((ok_quotes, failures))
    }
}

use crate::Resampling;
use crate::{Attribution, Borsa, MergeStrategy, Span};
use borsa_core::{BorsaConnector, BorsaError, HistoryRequest, HistoryResponse};
// use of HistoryProvider trait object occurs via method return; no import needed

type IndexedConnector = (usize, std::sync::Arc<dyn BorsaConnector>);
// Resampling plan applied to provider outputs to satisfy the requested cadence.
// Minutes is used for intraday up-aggregation (e.g., 2m from 1m).
enum ResamplePlan {
    Minutes(i64),
    Daily,
    Weekly,
}

type HistoryTaskResult = (
    usize,
    &'static str,
    Result<HistoryResponse, BorsaError>,
    Option<ResamplePlan>,
);
type HistoryOk = (usize, &'static str, HistoryResponse);
type CollectedHistory = (Vec<HistoryOk>, Vec<BorsaError>);

fn choose_effective_interval(
    supported: &[borsa_core::Interval],
    requested: borsa_core::Interval,
) -> Result<(borsa_core::Interval, Option<ResamplePlan>), BorsaError> {
    use borsa_core::Interval;

    // Exact support: pass-through
    if supported.contains(&requested) {
        return Ok((requested, None));
    }

    // Intraday request: prefer the largest supported divisor (coarsest <= requested)
    // to minimize fetched data volume. If no divisor exists, return unsupported so the
    // orchestrator can try another provider or surface the failure.
    if let Some(req_min) = requested.minutes() {
        let mut best_divisor: Option<(Interval, i64)> = None; // largest m dividing req_min
        for &s in supported {
            if let Some(m) = s.minutes()
                && m <= req_min
                && req_min % m == 0
                && best_divisor.as_ref().is_none_or(|&(_, bm)| m > bm)
            {
                best_divisor = Some((s, m));
            }
        }
        if let Some((eff, _)) = best_divisor {
            return Ok((eff, Some(ResamplePlan::Minutes(req_min))));
        }
        return Err(BorsaError::unsupported(
            "history interval (intraday too fine for provider)",
        ));
    }

    // Non-intraday: support graceful fallbacks where the router can emulate
    match requested {
        Interval::D1 => {
            // Prefer native daily; otherwise up-aggregate from the coarsest intraday interval
            if supported.contains(&Interval::D1) {
                Ok((Interval::D1, None))
            } else {
                let coarsest_intraday = supported
                    .iter()
                    .filter_map(|&iv| iv.minutes().map(|m| (iv, m)))
                    .max_by_key(|&(_, m)| m);
                if let Some((eff, _)) = coarsest_intraday {
                    Ok((eff, Some(ResamplePlan::Daily)))
                } else {
                    Err(BorsaError::unsupported(
                        "history interval (daily requires daily or intraday)",
                    ))
                }
            }
        }
        Interval::W1 => {
            // Prefer native weekly; fallback to daily or intraday with weekly resampling
            if supported.contains(&Interval::W1) {
                Ok((Interval::W1, None))
            } else if supported.contains(&Interval::D1) {
                Ok((Interval::D1, Some(ResamplePlan::Weekly)))
            } else {
                let coarsest_intraday = supported
                    .iter()
                    .filter_map(|&iv| iv.minutes().map(|m| (iv, m)))
                    .max_by_key(|&(_, m)| m);
                if let Some((eff, _)) = coarsest_intraday {
                    Ok((eff, Some(ResamplePlan::Weekly)))
                } else {
                    Err(BorsaError::unsupported(
                        "history interval (weekly requires weekly/daily/intraday)",
                    ))
                }
            }
        }
        // Generic handling for other calendar-based intervals (e.g., D5, M1, M3):
        // Pass through to the provider without emulation. If unsupported by the provider,
        // it will fail and be handled by the router's normal error flow.
        _ => Ok((requested, None)),
    }
}

impl Borsa {
    async fn fetch_joined_history(
        &self,
        eligible: &[(usize, std::sync::Arc<dyn BorsaConnector>)],
        inst: &borsa_core::Instrument,
        req_copy: HistoryRequest,
    ) -> Result<Vec<HistoryTaskResult>, BorsaError> {
        let make_future = || async {
            match self.cfg.merge_history_strategy {
                MergeStrategy::Deep => {
                    Self::parallel_history(eligible, inst, &req_copy, self.cfg.provider_timeout)
                        .await
                }
                MergeStrategy::Fallback => {
                    Self::sequential_history(
                        eligible.to_vec(),
                        inst,
                        req_copy,
                        self.cfg.provider_timeout,
                    )
                    .await
                }
            }
        };
        if let Some(deadline) = self.cfg.request_timeout {
            (tokio::time::timeout(deadline, make_future()).await)
                .map_or_else(|_| Err(BorsaError::request_timeout("history")), Ok)
        } else {
            Ok(make_future().await)
        }
    }

    fn finalize_history_results(
        &self,
        joined: Vec<HistoryTaskResult>,
        symbol: &str,
    ) -> Result<(HistoryResponse, Attribution), BorsaError> {
        let attempts = joined.len();
        let (mut results_ord, errors) = Self::collect_successes(joined);
        if results_ord.is_empty() {
            if errors.is_empty() {
                return Err(BorsaError::not_found(format!("history for {symbol}")));
            }
            if errors.len() == attempts
                && errors
                    .iter()
                    .all(|e| matches!(e, BorsaError::ProviderTimeout { .. }))
            {
                return Err(BorsaError::AllProvidersTimedOut {
                    capability: "history",
                });
            }
            return Err(BorsaError::AllProvidersFailed(errors));
        }

        self.order_results(&mut results_ord);
        let filtered_ord: Vec<HistoryOk> = self.filter_adjustedness(results_ord);
        let results: Vec<(&'static str, HistoryResponse)> =
            filtered_ord.into_iter().map(|(_, n, hr)| (n, hr)).collect();
        let attr = Self::build_attribution(&results, symbol);
        let mut merged = Self::merge_history_or_tag_connector_error(&results)?;
        self.apply_final_resample(&mut merged)?;
        Ok((merged, attr))
    }

    fn filter_adjustedness(&self, results_ord: Vec<HistoryOk>) -> Vec<HistoryOk> {
        if results_ord.is_empty() {
            return Vec::new();
        }
        if self.cfg.prefer_adjusted_history && results_ord.iter().any(|(_, _, hr)| hr.adjusted) {
            return results_ord
                .into_iter()
                .filter(|(_, _, hr)| hr.adjusted)
                .collect();
        }
        let target_adjusted = results_ord.first().is_some_and(|(_, _, hr)| hr.adjusted);
        results_ord
            .into_iter()
            .filter(|(_, _, hr)| hr.adjusted == target_adjusted)
            .collect()
    }

    fn merge_history_or_tag_connector_error(
        results: &[(&'static str, HistoryResponse)],
    ) -> Result<HistoryResponse, BorsaError> {
        if results.len() == 1 {
            return Ok(results.first().unwrap().1.clone());
        }
        match borsa_core::timeseries::merge::merge_history(results.iter().cloned().map(|(_, r)| r))
        {
            Ok(mut m) => {
                for c in &mut m.candles {
                    c.close_unadj = None;
                }
                Ok(m)
            }
            Err(borsa_core::BorsaError::Data(msg))
                if msg == "Connector provided mixed-currency history" =>
            {
                Err(Self::identify_faulty_provider(results))
            }
            Err(e) => Err(e),
        }
    }

    fn identify_faulty_provider(
        results: &[(&'static str, HistoryResponse)],
    ) -> borsa_core::BorsaError {
        use std::collections::HashMap;
        let mut per_provider_currency: HashMap<&'static str, Option<borsa_core::Currency>> =
            HashMap::new();
        for (name, hr) in results {
            let mut cur: Option<borsa_core::Currency> = None;
            let mut consistent = true;
            for c in &hr.candles {
                let oc = c.open.currency().clone();
                if let Some(prev) = &cur {
                    if prev != &oc
                        || oc != *c.high.currency()
                        || oc != *c.low.currency()
                        || oc != *c.close.currency()
                    {
                        consistent = false;
                        break;
                    }
                } else {
                    cur = Some(oc);
                }
            }
            per_provider_currency.insert(*name, if consistent { cur } else { None });
        }
        if let Some((bad_name, _)) = per_provider_currency.iter().find(|(_, v)| v.is_none()) {
            return borsa_core::BorsaError::Connector {
                connector: (*bad_name).to_string(),
                msg: "Provider returned inconsistent currency data".to_string(),
            };
        }
        let mut counts: HashMap<borsa_core::Currency, usize> = HashMap::new();
        for v in per_provider_currency.values() {
            if let Some(cur) = v.clone() {
                *counts.entry(cur).or_insert(0) += 1;
            }
        }
        let majority = counts.into_iter().max_by_key(|(_, c)| *c).map(|(k, _)| k);
        if let Some(maj) = majority
            && let Some((bad_name, _)) = per_provider_currency
                .into_iter()
                .find(|(_, v)| v.as_ref() != Some(&maj))
        {
            return borsa_core::BorsaError::Connector {
                connector: bad_name.to_string(),
                msg: "Provider returned inconsistent currency data".to_string(),
            };
        }
        let fallback = results.last().map_or("unknown", |(n, _)| *n);
        borsa_core::BorsaError::Connector {
            connector: fallback.to_string(),
            msg: "Provider returned inconsistent currency data".to_string(),
        }
    }
    /// Fetch historical OHLCV and actions for an instrument.
    ///
    /// Behavior and trade-offs:
    /// - Provider selection is guided by per-symbol and per-kind preferences.
    /// - The effective interval is chosen per provider: if the requested interval is
    ///   unsupported, a largest supported divisor is used and later resampled when
    ///   possible. This may smooth irregularities but can lose native cadence details.
    /// - Merge behavior depends on [`MergeStrategy`]: `Deep` backfills gaps across
    ///   providers (more complete, more requests) while `Fallback` returns the first
    ///   non-empty dataset (fewer requests, potentially missing data).
    /// - Resampling as configured on the builder may clear provider-specific
    ///   `unadjusted_close` fields to avoid ambiguity across cadences and providers.
    /// - Preference for adjusted history can change ordering among successful sources
    ///   to reduce splits/dividend discontinuities at the cost of deviating from raw
    ///   close values.
    /// # Errors
    /// Returns an error if all eligible providers fail or if no provider supports
    /// the requested capability for the instrument.
    /// # Errors
    /// Returns an error if no eligible provider succeeds or none support the capability.
    pub async fn history(
        &self,
        inst: &borsa_core::Instrument,
        req: HistoryRequest,
    ) -> Result<HistoryResponse, BorsaError> {
        let (merged, _attr) = self.history_with_attribution(inst, req).await?;
        Ok(merged)
    }

    /// Fetch history and include attribution showing provider spans used in the merge.
    ///
    /// Additional details:
    /// - Returns both the merged `HistoryResponse` and an [`Attribution`] that lists
    ///   continuous timestamp spans contributed by each provider after de-duplication.
    /// - When resampling is applied (forced or via auto-subdaily), `unadjusted_close` is
    ///   cleared in the merged output to prevent mixing raw and adjusted semantics.
    /// - In `Fallback` mode, providers are tried sequentially until a non-empty
    ///   result is found; empty-but-OK responses are allowed to continue the search.
    /// - On overall failure, returns `NotFound` only when no non-`NotFound` errors
    ///   occurred (i.e., all attempts were `NotFound` or empty). If every attempt
    ///   timed out, returns `AllProvidersTimedOut`; otherwise returns
    ///   `AllProvidersFailed` with aggregated errors.
    ///
    /// # Panics
    /// Panics if reconstructing an intermediate `HistoryRequest` fails during
    /// effective-interval selection. These are expected to be valid given
    /// the original request.
    /// # Errors
    /// Returns an error if all eligible providers fail or if no provider supports
    /// the requested capability for the instrument.
    pub async fn history_with_attribution(
        &self,
        inst: &borsa_core::Instrument,
        req: HistoryRequest,
    ) -> Result<(HistoryResponse, Attribution), BorsaError> {
        // Request types validate on construction
        let eligible = self.eligible_history_connectors(inst)?;
        let req_copy = req;
        let joined = self.fetch_joined_history(&eligible, inst, req_copy).await?;
        self.finalize_history_results(joined, inst.symbol_str())
    }
}

impl Borsa {
    fn eligible_history_connectors(
        &self,
        inst: &borsa_core::Instrument,
    ) -> Result<Vec<IndexedConnector>, BorsaError> {
        let ordered = self.ordered(inst);
        let mut eligible: Vec<(usize, std::sync::Arc<dyn BorsaConnector>)> = Vec::new();
        for (idx, c) in ordered.into_iter().enumerate() {
            if c.supports_kind(*inst.kind()) && c.as_history_provider().is_some() {
                eligible.push((idx, c));
            }
        }
        if eligible.is_empty() {
            return Err(BorsaError::unsupported("history"));
        }
        Ok(eligible)
    }

    async fn parallel_history(
        eligible: &[(usize, std::sync::Arc<dyn BorsaConnector>)],
        inst: &borsa_core::Instrument,
        req_copy: &HistoryRequest,
        provider_timeout: std::time::Duration,
    ) -> Vec<HistoryTaskResult> {
        let tasks = eligible.iter().map(|(idx, c)| {
            Self::spawn_history_task(*idx, c.clone(), inst.clone(), req_copy, provider_timeout)
        });
        futures::future::join_all(tasks).await
    }

    fn build_effective_request(
        c: &std::sync::Arc<dyn BorsaConnector>,
        kind: borsa_core::AssetKind,
        req_copy: &HistoryRequest,
    ) -> Result<(HistoryRequest, Option<ResamplePlan>), BorsaError> {
        let supported = c
            .as_history_provider()
            .expect("checked is_some above")
            .supported_history_intervals(kind)
            .to_vec();
        let (effective_interval, resample_plan) =
            choose_effective_interval(&supported, req_copy.interval())?;
        // Preserve all ancillary flags and timeframe; only adjust the interval using the builder.
        let mut b = borsa_core::HistoryRequestBuilder::default();
        if let Some(r) = req_copy.range() {
            b = b.range(r);
        } else if let Some((s, e)) = req_copy.period() {
            b = b.period(s, e);
        }
        b = b.interval(effective_interval);
        b = b.include_prepost(req_copy.include_prepost());
        b = b.include_actions(req_copy.include_actions());
        b = b.auto_adjust(req_copy.auto_adjust());
        b = b.keepna(req_copy.keepna());
        let eff_req = b.build()?;
        Ok((eff_req, resample_plan))
    }

    fn spawn_history_task(
        idx: usize,
        c: std::sync::Arc<dyn BorsaConnector>,
        inst: borsa_core::Instrument,
        req_copy: &HistoryRequest,
        provider_timeout: std::time::Duration,
    ) -> impl std::future::Future<
        Output = (
            usize,
            &'static str,
            Result<HistoryResponse, BorsaError>,
            Option<ResamplePlan>,
        ),
    > {
        let kind = *inst.kind();
        async move {
            let (eff_req, resample_target_min) =
                match Self::build_effective_request(&c, kind, req_copy) {
                    Ok(v) => v,
                    Err(e) => return (idx, c.name(), Err(e), None),
                };
            let fut = c
                .as_history_provider()
                .expect("checked is_some above")
                .history(&inst, eff_req);
            let resp =
                Self::provider_call_with_timeout(c.name(), "history", provider_timeout, fut).await;
            (idx, c.name(), resp, resample_target_min)
        }
    }

    async fn sequential_history(
        eligible: Vec<IndexedConnector>,
        inst: &borsa_core::Instrument,
        req_copy: HistoryRequest,
        provider_timeout: std::time::Duration,
    ) -> Vec<HistoryTaskResult> {
        let mut results = Vec::new();
        for (idx, c) in eligible {
            let (eff_req, resample_target_min) =
                match Self::build_effective_request(&c, *inst.kind(), &req_copy) {
                    Ok(v) => v,
                    Err(e) => {
                        let result = (idx, c.name(), Err(e), None);
                        results.push(result);
                        continue;
                    }
                };
            let fut = c
                .as_history_provider()
                .expect("checked is_some above")
                .history(inst, eff_req);
            let resp =
                Self::provider_call_with_timeout(c.name(), "history", provider_timeout, fut).await;
            let result = (idx, c.name(), resp, resample_target_min);
            if let Ok(ref hr) = result.2
                && !hr.candles.is_empty()
            {
                results.push(result);
                break;
            }
            results.push(result);
        }
        results
    }

    fn collect_successes(joined: Vec<HistoryTaskResult>) -> CollectedHistory {
        let mut results_ord: Vec<HistoryOk> = Vec::new();
        let mut errors: Vec<BorsaError> = Vec::new();

        for (idx, name, res, resample_target_min) in joined {
            match res {
                Ok(mut hr) if !hr.candles.is_empty() => {
                    if let Some(plan) = resample_target_min {
                        match plan {
                            ResamplePlan::Minutes(mins) => {
                                match borsa_core::timeseries::resample::resample_to_minutes_with_meta(
                                    std::mem::take(&mut hr.candles),
                                    mins,
                                    hr.meta.as_ref(),
                                ) {
                                    Ok(c) => hr.candles = c,
                                    Err(e) => {
                                        errors.push(crate::core::tag_err(name, e));
                                        continue;
                                    }
                                }
                                for c in &mut hr.candles {
                                    c.close_unadj = None;
                                }
                            }
                            ResamplePlan::Daily => {
                                match borsa_core::timeseries::resample::resample_to_daily_with_meta(
                                    std::mem::take(&mut hr.candles),
                                    hr.meta.as_ref(),
                                ) {
                                    Ok(c) => hr.candles = c,
                                    Err(e) => {
                                        errors.push(crate::core::tag_err(name, e));
                                        continue;
                                    }
                                }
                                for c in &mut hr.candles {
                                    c.close_unadj = None;
                                }
                            }
                            ResamplePlan::Weekly => {
                                match borsa_core::timeseries::resample::resample_to_weekly_with_meta(
                                    std::mem::take(&mut hr.candles),
                                    hr.meta.as_ref(),
                                ) {
                                    Ok(c) => hr.candles = c,
                                    Err(e) => {
                                        errors.push(crate::core::tag_err(name, e));
                                        continue;
                                    }
                                }
                                for c in &mut hr.candles {
                                    c.close_unadj = None;
                                }
                            }
                        }
                    }
                    results_ord.push((idx, name, hr));
                }
                Ok(_) | Err(borsa_core::BorsaError::NotFound { .. }) => {}
                Err(e) => errors.push(crate::core::tag_err(name, e)),
            }
        }
        (results_ord, errors)
    }

    fn order_results(&self, results_ord: &mut Vec<HistoryOk>) {
        if self.cfg.prefer_adjusted_history {
            // Prefer adjusted datasets while preserving original connector precedence within groups.
            // Use a single composite key to avoid relying on sort stability.
            results_ord.sort_by_key(|(idx, _, hr)| (!hr.adjusted, *idx));
        } else {
            results_ord.sort_by_key(|(idx, _, _)| *idx);
        }
    }

    fn build_attribution(results: &[(&'static str, HistoryResponse)], symbol: &str) -> Attribution {
        let mut attr = Attribution::new(symbol.to_string());
        let mut seen = std::collections::BTreeSet::<chrono::DateTime<chrono::Utc>>::new();
        for (name, hr) in results {
            // Determine contiguity based on the provider's effective cadence.
            let step_opt = borsa_core::timeseries::infer::estimate_step_seconds(hr.candles.clone());
            let mut by_ts = hr.candles.iter().collect::<Vec<_>>();
            by_ts.sort_by_key(|c| c.ts);

            let mut run_start: Option<i64> = None;
            let mut last_kept_ts: Option<i64> = None;
            for c in by_ts {
                if !seen.insert(c.ts) {
                    // First-wins: skip timestamps already attributed to earlier providers.
                    continue;
                }
                let ts_sec = c.ts.timestamp();
                match (run_start, last_kept_ts, step_opt) {
                    (None, _, _) => {
                        run_start = Some(ts_sec);
                        last_kept_ts = Some(ts_sec);
                    }
                    (Some(_), Some(last), Some(step)) if ts_sec - last == step => {
                        // Continues the current run
                        last_kept_ts = Some(ts_sec);
                    }
                    (Some(start), Some(last), Some(_)) => {
                        // Gap detected: close previous run and start a new one
                        attr.push((*name, Span { start, end: last }));
                        run_start = Some(ts_sec);
                        last_kept_ts = Some(ts_sec);
                    }
                    (Some(start), Some(last), None) => {
                        // Unknown cadence: treat every kept point as its own run boundary
                        attr.push((*name, Span { start, end: last }));
                        run_start = Some(ts_sec);
                        last_kept_ts = Some(ts_sec);
                    }
                    _ => {}
                }
            }
            if let (Some(start), Some(end)) = (run_start, last_kept_ts) {
                attr.push((*name, Span { start, end }));
            }
        }
        attr
    }

    fn apply_final_resample(&self, merged: &mut HistoryResponse) -> Result<(), BorsaError> {
        let will_resample = if !matches!(self.cfg.resampling, Resampling::None) {
            true
        } else if self.cfg.auto_resample_subdaily_to_daily {
            borsa_core::timeseries::infer::is_subdaily(&merged.candles)
        } else {
            false
        };
        if will_resample {
            for c in &mut merged.candles {
                c.close_unadj = None;
            }
        }

        if matches!(self.cfg.resampling, Resampling::Weekly) {
            let new_candles = borsa_core::timeseries::resample::resample_to_weekly_with_meta(
                std::mem::take(&mut merged.candles),
                merged.meta.as_ref(),
            )?;
            merged.candles = new_candles;
        } else if matches!(self.cfg.resampling, Resampling::Daily)
            || (self.cfg.auto_resample_subdaily_to_daily
                && borsa_core::timeseries::infer::is_subdaily(&merged.candles))
        {
            let new_candles = borsa_core::timeseries::resample::resample_to_daily_with_meta(
                std::mem::take(&mut merged.candles),
                merged.meta.as_ref(),
            )?;
            merged.candles = new_candles;
        }
        Ok(())
    }
}

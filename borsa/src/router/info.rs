use crate::Borsa;
use borsa_core::{BorsaError, Isin};

type ProfileFields = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<borsa_core::Address>,
    Option<String>,
    Option<borsa_core::FundKind>,
);

fn append_actionable(errors: &mut Vec<BorsaError>, err: BorsaError) {
    match err {
        BorsaError::AllProvidersFailed(list) => {
            for inner in list {
                append_actionable(errors, inner);
            }
        }
        BorsaError::Unsupported { .. } | BorsaError::NotFound { .. } => {}
        other => errors.push(other),
    }
}

impl Borsa {
    /// Build a comprehensive `Info` record by composing multiple data sources.
    ///
    /// Behavior and trade-offs:
    /// - Executes `profile`, `quote`, and `isin` concurrently, then synthesizes a best-effort
    ///   view. Individual subcalls may fail without failing the overall result.
    /// - `isin` is derived from an explicit provider first and then from `profile`
    ///   when available, providing resilience at the cost of potentially stale data.
    /// - The returned `Info` favors quote fields for price/market state and profile
    ///   for descriptive text; missing fields remain `None` rather than erroring.
    /// # Errors
    /// Returns an error only if task join fails unexpectedly.
    /// Otherwise, succeeds and includes per-source errors in the `errors` field.
    pub async fn info(
        &self,
        inst: &borsa_core::Instrument,
    ) -> Result<borsa_core::InfoReport, BorsaError> {
        let (profile_res, quote_res, isin_res) =
            tokio::join!(self.profile(inst), self.quote(inst), self.isin(inst));

        // Collect errors with flattening of AllProvidersFailed for transparency
        let mut errors: Vec<BorsaError> = Vec::new();
        let mut push_err = |e: BorsaError| append_actionable(&mut errors, e);

        let profile = match profile_res {
            Ok(v) => Some(v),
            Err(e) => {
                push_err(e);
                None
            }
        };
        let quote = match quote_res {
            Ok(v) => Some(v),
            Err(e) => {
                push_err(e);
                None
            }
        };
        let explicit_isin: Option<Isin> = match isin_res {
            Ok(v) => v,
            Err(e) => {
                push_err(e);
                None
            }
        };
        let isin_val = Self::pick_isin(profile.as_ref(), explicit_isin);

        let (name, _sector, _industry, _website, _summary, _address, _family, _fund_kind) =
            Self::pick_profile_fields(profile.as_ref());

        let name_field = quote.as_ref().and_then(|q| q.shortname.clone()).or(name);
        let currency = quote.as_ref().and_then(|q| {
            q.price
                .as_ref()
                .or(q.previous_close.as_ref())
                .map(|m| m.currency().clone())
        });
        Ok(borsa_core::InfoReport {
            symbol: inst.symbol().clone(),
            info: Some(borsa_core::Info {
                symbol: inst.symbol().clone(),
                name: name_field,
                isin: isin_val,
                exchange: quote.as_ref().and_then(|q| q.exchange.clone()),
                market_state: quote.as_ref().and_then(|q| q.market_state),
                currency,
                last: quote.as_ref().and_then(|q| q.price.clone()),
                open: None,
                high: None,
                low: None,
                previous_close: quote.as_ref().and_then(|q| q.previous_close.clone()),
                day_range_low: None,
                day_range_high: None,
                fifty_two_week_low: None,
                fifty_two_week_high: None,
                volume: None,
                average_volume: None,
                market_cap: None,
                shares_outstanding: None,
                eps_ttm: None,
                pe_ttm: None,
                dividend_yield: None,
                ex_dividend_date: None,
                as_of: None,
            }),
            warnings: errors.into_iter().map(|e| e.to_string()).collect(),
        })
    }

    fn pick_isin(profile: Option<&borsa_core::Profile>, explicit: Option<Isin>) -> Option<Isin> {
        explicit.or_else(|| profile.and_then(|p| p.isin().cloned()))
    }

    fn pick_profile_fields(profile: Option<&borsa_core::Profile>) -> ProfileFields {
        profile.map_or(
            (None, None, None, None, None, None, None, None),
            |p| match p {
                borsa_core::Profile::Company(c) => (
                    Some(c.name.clone()),
                    c.sector.clone(),
                    c.industry.clone(),
                    c.website.clone(),
                    c.summary.clone(),
                    c.address.clone(),
                    None,
                    None,
                ),
                borsa_core::Profile::Fund(f) => (
                    Some(f.name.clone()),
                    None,
                    None,
                    None,
                    None,
                    None,
                    f.family.clone(),
                    Some(f.kind.clone()),
                ),
            },
        )
    }

    /// Lightweight `FastInfo` derived primarily from quotes.
    ///
    /// Behavior and trade-offs:
    /// - Uses the point-in-time quote and derives the latest price from the
    ///   `price` field, falling back to `previous_close` when absent.
    /// - Fails with a data error if neither is present, making it suitable for
    ///   latency-sensitive paths where completeness is secondary to availability.
    /// # Errors
    /// Returns an error if the quote lacks both last and previous price.
    pub async fn fast_info(
        &self,
        inst: &borsa_core::Instrument,
    ) -> Result<borsa_core::FastInfo, BorsaError> {
        let q = self.quote(inst).await?;
        let last = q
            .price
            .clone()
            .or_else(|| q.previous_close.clone())
            .ok_or_else(|| {
                BorsaError::Data(format!(
                    "quote for {} missing last/previous price",
                    inst.symbol()
                ))
            })?;
        let currency = last.currency().clone();

        Ok(borsa_core::FastInfo {
            symbol: inst.symbol().clone(),
            name: q.shortname,
            exchange: q.exchange,
            market_state: q.market_state,
            currency: Some(currency),
            last: Some(last),
            previous_close: q.previous_close,
        })
    }
}

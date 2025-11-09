use crate::Borsa;
use borsa_core::{
    BorsaError, Capability, DownloadEntry, DownloadReport, DownloadResponse, HistoryRequest,
    HistoryResponse, Instrument, Range,
};
use chrono::DateTime;
use std::collections::HashSet;

// Validate that all instruments have unique identity keys (scheme-agnostic).
fn validate_unique_instruments(insts: &[Instrument]) -> Result<(), BorsaError> {
    let mut seen: HashSet<String> = HashSet::new();
    for inst in insts {
        let key = inst.id().unique_key().into_owned();
        if !seen.insert(key.clone()) {
            // Preserve symbol-centric message for securities to avoid breaking callers/tests.
            let label = match inst.id() {
                borsa_core::IdentifierScheme::Security(sec) => sec.symbol.as_str().to_string(),
                borsa_core::IdentifierScheme::Prediction(_) => key,
            };
            return Err(BorsaError::InvalidArg(format!(
                "duplicate symbol '{label}' in instruments list"
            )));
        }
    }
    Ok(())
}

/// Builder to orchestrate bulk history downloads for multiple symbols.
pub struct DownloadBuilder<'a> {
    pub(crate) borsa: &'a Borsa,
    pub(crate) instruments: Vec<Instrument>,
    // Defer building a validated HistoryRequest until run(), to avoid panics on input.
    pub(crate) range: Option<Range>,
    pub(crate) period: Option<(i64, i64)>,
    pub(crate) interval: borsa_core::Interval,
}

impl<'a> DownloadBuilder<'a> {
    /// Create a new builder bound to a `Borsa` instance.
    ///
    /// Behavior:
    /// - Starts with an empty instrument list.
    /// - Defers validation of range/period/interval until `run()`.
    #[must_use]
    pub const fn new(borsa: &'a Borsa) -> Self {
        Self {
            borsa,
            instruments: Vec::new(),
            range: Some(Range::M6),
            period: None,
            interval: borsa_core::Interval::D1,
        }
    }

    /// Replace the instruments list.
    ///
    /// Trade-offs: Replaces any previously added instruments; use `add_instrument`
    /// if you need to append.
    ///
    /// # Errors
    /// Returns an error if duplicate instruments are detected in the provided list.
    pub fn instruments(mut self, insts: &[Instrument]) -> Result<Self, BorsaError> {
        validate_unique_instruments(insts)?;

        self.instruments = insts.to_vec();
        Ok(self)
    }

    /// Add a single instrument to the list.
    ///
    /// # Errors
    /// Returns an error if the instrument's symbol already exists in the list.
    ///
    /// # Panics
    /// Panics only if an internal invariant is broken whereby the just-pushed
    /// instrument is missing; this cannot occur in normal use.
    pub fn add_instrument(mut self, inst: Instrument) -> Result<Self, BorsaError> {
        let mut combined = self.instruments.clone();
        combined.push(inst);
        if validate_unique_instruments(&combined).is_err() {
            let last = combined.last().expect("pushed instrument exists");
            let sym = match last.id() {
                borsa_core::IdentifierScheme::Security(sec) => sec.symbol.as_str().to_string(),
                borsa_core::IdentifierScheme::Prediction(_) => last.id().unique_key().into_owned(),
            };
            return Err(BorsaError::InvalidArg(format!(
                "duplicate symbol '{sym}' already exists in instruments list"
            )));
        }

        self.instruments = combined;
        Ok(self)
    }

    /// Set a logical lookback range and clear any explicit period.
    ///
    /// Behavior: Mutually exclusive with `period`; setting this clears an existing
    /// explicit [start, end).
    #[must_use]
    pub const fn range(mut self, range: Range) -> Self {
        self.range = Some(range);
        self.period = None;
        self
    }

    /// Set an explicit period [start, end) and clear any logical range.
    ///
    /// Behavior: Mutually exclusive with `range`; setting this clears an existing
    /// logical range.
    #[must_use]
    pub const fn period(mut self, start: i64, end: i64) -> Self {
        self.period = Some((start, end));
        self.range = None;
        self
    }

    /// Select the desired history interval.
    #[must_use]
    pub const fn interval(mut self, interval: borsa_core::Interval) -> Self {
        self.interval = interval;
        self
    }

    /// Execute the download across eligible providers and aggregate results.
    ///
    /// Behavior and trade-offs:
    /// - Validates the request and then concurrently fetches per-symbol history using
    ///   the same merge/resample rules as `Borsa::history_with_attribution`.
    /// - Populates the returned [`DownloadReport`] with a [`borsa_core::DownloadResponse`]
    ///   containing per-symbol candles, actions, and metadata keyed by symbol when at
    ///   least one instrument succeeds.
    /// - Partial failures populate the `warnings` vector with `{symbol}: {error}` entries
    ///   without aborting the entire batch.
    /// # Errors
    /// Returns an error only if no instruments are specified or if an overall
    /// request-level timeout elapses.
    pub async fn run(self) -> Result<DownloadReport, BorsaError> {
        if self.instruments.is_empty() {
            return Err(BorsaError::InvalidArg(
                "no instruments specified for download".into(),
            ));
        }

        // Defensive check for duplicates (should not happen if using the builder correctly)
        validate_unique_instruments(&self.instruments)?;

        // Build a validated HistoryRequest now; convert timestamp seconds safely.
        let req: HistoryRequest = if let Some((start, end)) = self.period {
            let start_dt = DateTime::from_timestamp(start, 0).ok_or_else(|| {
                BorsaError::InvalidArg(format!("invalid start timestamp: {start}"))
            })?;
            let end_dt = DateTime::from_timestamp(end, 0)
                .ok_or_else(|| BorsaError::InvalidArg(format!("invalid end timestamp: {end}")))?;
            HistoryRequest::try_from_period(start_dt, end_dt, self.interval)?
        } else {
            let range = self.range.unwrap_or(Range::M6);
            HistoryRequest::try_from_range(range, self.interval)?
        };

        let tasks = self.instruments.iter().map(|inst| {
            let borsa = self.borsa;
            let req = req.clone();
            let inst = inst.clone();
            async move {
                match borsa.history_with_attribution(&inst, req).await {
                    Ok((hr, _attr)) => (inst, Ok(hr)),
                    Err(e) => (inst, Err(e)),
                }
            }
        });

        // Apply optional request-level deadline across the fan-out
        let joined: Vec<(Instrument, Result<HistoryResponse, BorsaError>)> =
            match crate::router::util::join_with_deadline(tasks, self.borsa.cfg.request_timeout)
                .await
            {
                Ok(v) => v,
                Err(_) => {
                    return Err(BorsaError::request_timeout(
                        Capability::DownloadHistory.to_string(),
                    ));
                }
            };

        let mut entries: Vec<DownloadEntry> = Vec::new();
        let mut had_success = false;
        let mut warnings: Vec<BorsaError> = Vec::new();
        for (instrument, result) in joined {
            match result {
                Ok(resp) => {
                    had_success = true;
                    entries.push(DownloadEntry {
                        instrument,
                        history: resp,
                    });
                }
                Err(e) => {
                    // Preserve the original error, which is already connector-tagged upstream.
                    warnings.push(e);
                }
            }
        }

        let response = if had_success {
            Some(DownloadResponse { entries })
        } else {
            None
        };

        Ok(DownloadReport { response, warnings })
    }
}

impl Borsa {
    /// Begin building a bulk download request.
    ///
    /// Typical usage: chain `instruments`/`range`/`interval` then call `run()`.
    #[must_use]
    pub const fn download(&'_ self) -> DownloadBuilder<'_> {
        DownloadBuilder::new(self)
    }
}

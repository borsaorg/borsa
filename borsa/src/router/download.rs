use crate::Borsa;
use borsa_core::{
    BorsaError, DownloadEntry, DownloadReport, DownloadResponse, HistoryRequest, Instrument,
};
use std::collections::HashSet;

/// Builder to orchestrate bulk history downloads for multiple symbols.
pub struct DownloadBuilder<'a> {
    pub(crate) borsa: &'a Borsa,
    pub(crate) instruments: Vec<Instrument>,
    // Defer building a validated HistoryRequest until run(), to avoid panics on input.
    pub(crate) range: Option<borsa_core::Range>,
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
            range: Some(borsa_core::Range::M6),
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
    /// Returns an error if duplicate symbols are detected in the provided instruments.
    pub fn instruments(mut self, insts: &[Instrument]) -> Result<Self, BorsaError> {
        // Check for duplicate symbols
        let mut seen = HashSet::new();
        for inst in insts {
            let symbol = inst.symbol().to_string();
            if !seen.insert(symbol.clone()) {
                return Err(BorsaError::InvalidArg(format!(
                    "duplicate symbol '{symbol}' in instruments list"
                )));
            }
        }

        self.instruments = insts.to_vec();
        Ok(self)
    }

    /// Add a single instrument to the list.
    ///
    /// # Errors
    /// Returns an error if the instrument's symbol already exists in the list.
    pub fn add_instrument(mut self, inst: Instrument) -> Result<Self, BorsaError> {
        // Check for duplicate symbol
        if self
            .instruments
            .iter()
            .any(|existing| existing.symbol_str() == inst.symbol_str())
        {
            return Err(BorsaError::InvalidArg(format!(
                "duplicate symbol '{}' already exists in instruments list",
                inst.symbol()
            )));
        }

        self.instruments.push(inst);
        Ok(self)
    }

    /// Set a logical lookback range and clear any explicit period.
    ///
    /// Behavior: Mutually exclusive with `period`; setting this clears an existing
    /// explicit [start, end).
    #[must_use]
    pub const fn range(mut self, range: borsa_core::Range) -> Self {
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
        let mut seen = HashSet::new();
        for inst in &self.instruments {
            let symbol = inst.symbol().to_string();
            if !seen.insert(symbol.clone()) {
                return Err(BorsaError::InvalidArg(format!(
                    "duplicate symbol '{symbol}' detected in instruments list"
                )));
            }
        }

        // Build a validated HistoryRequest now; convert timestamp seconds safely.
        let req: HistoryRequest = if let Some((start, end)) = self.period {
            let start_dt = chrono::DateTime::from_timestamp(start, 0).ok_or_else(|| {
                BorsaError::InvalidArg(format!("invalid start timestamp: {start}"))
            })?;
            let end_dt = chrono::DateTime::from_timestamp(end, 0)
                .ok_or_else(|| BorsaError::InvalidArg(format!("invalid end timestamp: {end}")))?;
            HistoryRequest::try_from_period(start_dt, end_dt, self.interval)?
        } else {
            let range = self.range.unwrap_or(borsa_core::Range::M6);
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
        let joined: Vec<(Instrument, Result<borsa_core::HistoryResponse, BorsaError>)> =
            if let Some(deadline) = self.borsa.cfg.request_timeout {
                match tokio::time::timeout(deadline, futures::future::join_all(tasks)).await {
                    Ok(v) => v,
                    Err(_) => return Err(BorsaError::request_timeout("download:history")),
                }
            } else {
                futures::future::join_all(tasks).await
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

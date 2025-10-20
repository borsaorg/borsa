/// Generate a router async method that selects providers, applies kind filters,
/// and calls a single-provider method. Handles not-found fallbacks via orchestrator.
///
/// Notes on `not_found` label:
/// - Pass a noun only (e.g., "quote", "holders", "analysis").
/// - The orchestrator formats the final error as "{label} for {SYMBOL}".
/// - Do not include the word "for" in the label.
#[macro_export]
macro_rules! borsa_router_method {
    (
        $(#[$meta:meta])*
        method: $name:ident( $inst_ident:ident : $inst_ty:ty $(, $arg_ident:ident : $arg_ty:ty )* ) -> $ret:ty,
        provider: $provider:ident,
        accessor: $accessor:ident,
        capability: $capability:expr,
        not_found: $not_found:expr,
        call: $call_name:ident( $call_first:ident $(, $call_rest:ident )* )
    ) => {
        $(#[$meta])*
        #[cfg_attr(
            feature = "tracing",
            tracing::instrument(
                target = "borsa::router",
                skip(self $(, $arg_ident)*),
                fields(symbol = %$inst_ident.symbol()),
            )
        )]
        ///
        /// # Errors
        /// Returns an error if no eligible provider succeeds or none support the capability.
        pub async fn $name(
            &self,
            $inst_ident: $inst_ty,
            $( $arg_ident: $arg_ty ),*
        ) -> Result<$ret, borsa_core::BorsaError> {
            self.fetch_single(
                $inst_ident,
                $capability,
                $not_found,
                move |c, i| {
                    if !c.supports_kind(*i.kind()) {
                        return None;
                    }
                    let c2 = c.clone();
                    if let Some(_) = c2.$accessor() {
                        Some({
                            let i2 = i.clone();
                            $( let $arg_ident = $arg_ident.clone(); )*
                            async move {
                                if let Some(p) = c2.$accessor() {
                                    p.$call_name(&i2 $(, $call_rest )*).await
                                } else {
                                    Err(borsa_core::BorsaError::connector(c2.name(), concat!("missing ", $capability, " capability during call")))
                                }
                            }
                        })
                    } else {
                        None
                    }
                },
            )
            .await
        }
    };
}

/// Generate a router search method that queries providers concurrently, de-dups
/// results by symbol, and applies an optional limit.
#[macro_export]
macro_rules! borsa_router_search {
    (
        $(#[$meta:meta])*
        method: $name:ident( $req_ident:ident : $req_ty:ty ) -> $ret:ty,
        accessor: $accessor:ident,
        capability: $capability:expr,
        call: $call_name:ident( $call_arg:ident )
    ) => {
        $(#[$meta])*
        #[cfg_attr(
            feature = "tracing",
            tracing::instrument(
                target = "borsa::router",
                skip(self, $req_ident),
                fields(kind = ?$req_ident.kind(), limit = $req_ident.limit()),
            )
        )]
        ///
        /// # Errors
        /// Returns an error if no provider produced any results and at least one provider
        /// failed (e.g., timeouts, server errors). Provider-specific failures are otherwise
        /// aggregated in `errors`. Also returns an error on overall request timeout.
        pub async fn $name(
            &self,
            $req_ident: $req_ty,
        ) -> Result<borsa_core::SearchReport, borsa_core::BorsaError> {
            // Request type validates on construction

            let ordered = self.ordered_for_kind($req_ident.kind());

            let req_copy = $req_ident.clone();
            let call_timeout = self.cfg.provider_timeout;
            let tasks = ordered.into_iter().map(|c| {
                let r = req_copy.clone();
                async move {
                    let name = c.name();
                    if r.kind().is_some_and(|k| !c.supports_kind(k)) {
                        return (name, false, Ok(borsa_core::SearchResponse { results: vec![] }));
                    }
                    if let Some(p) = c.$accessor() {
                        let res = $crate::Borsa::provider_call_with_timeout(
                            name,
                            $capability,
                            call_timeout,
                            p.$call_name(r),
                        )
                        .await;
                        (name, true, res)
                    } else {
                        (name, false, Ok(borsa_core::SearchResponse { results: vec![] }))
                    }
                }
            });

            // Apply optional request-level timeout if configured
            let joined = if let Some(deadline) = self.cfg.request_timeout {
                match tokio::time::timeout(deadline, futures::future::join_all(tasks)).await {
                    Ok(v) => v,
                    Err(_) => return Err(borsa_core::BorsaError::request_timeout($capability)),
                }
            } else {
                futures::future::join_all(tasks).await
            };

            let mut merged: Vec<borsa_core::SearchResult> = Vec::new();
            let mut seen = std::collections::BTreeSet::<String>::new();
            let mut errors: Vec<borsa_core::BorsaError> = Vec::new();
            let mut attempted_any = false;
            for (name, attempted, res) in joined {
                if attempted {
                    attempted_any = true;
                }
                match res {
                    Ok(sr) => {
                        if attempted {
                            for item in sr.results.into_iter() {
                                if seen.insert(item.symbol.as_str().to_string()) {
                                    merged.push(item);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        if attempted {
                            if let borsa_core::BorsaError::AllProvidersFailed(v) = e {
                                errors.extend(v);
                            } else {
                                errors.push($crate::core::tag_err(name, e));
                            }
                        }
                    }
                }
            }

            if !attempted_any {
                return Err(borsa_core::BorsaError::unsupported($capability));
            }

            if let Some(limit) = $req_ident.limit()
                && merged.len() > limit
            {
                merged.truncate(limit);
            }

            if merged.is_empty() && !errors.is_empty() {
                return Err(borsa_core::BorsaError::AllProvidersFailed(errors));
            }

            Ok(borsa_core::SearchReport { response: Some(borsa_core::SearchResponse { results: merged }), warnings: Vec::new() })
        }
    };
}

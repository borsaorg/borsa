use borsa_core::{BorsaError, Capability};

/// Join a collection of tasks and apply an optional request-level deadline.
///
/// This wraps `futures::future::join_all(tasks)` with `crate::core::with_request_deadline`.
/// On timeout, the inner helper returns `BorsaError::RequestTimeout("request")` which
/// call sites can remap to a more specific capability label as needed.
pub async fn join_with_deadline<I, F, T>(
    tasks: I,
    deadline: Option<std::time::Duration>,
) -> Result<Vec<T>, BorsaError>
where
    I: IntoIterator<Item = F>,
    F: core::future::Future<Output = T>,
{
    crate::core::with_request_deadline(deadline, futures::future::join_all(tasks)).await
}

/// Collapse a set of provider errors into a uniform `BorsaError` outcome.
///
/// Rules:
/// - If `attempted_any` is false → `Unsupported(capability)`.
/// - If all errors are `ProviderTimeout` → `AllProvidersTimedOut(capability)`.
/// - If `not_found_what` is `Some` and all errors are `NotFound` → `NotFound(what)`.
/// - Else → `AllProvidersFailed(errors)`.
pub fn collapse_errors(
    capability: Capability,
    attempted_any: bool,
    errors: Vec<BorsaError>,
    not_found_what: Option<String>,
) -> BorsaError {
    if !attempted_any {
        return BorsaError::unsupported(capability.to_string());
    }
    if !errors.is_empty()
        && errors
            .iter()
            .all(|e| matches!(e, BorsaError::ProviderTimeout { .. }))
    {
        return BorsaError::AllProvidersTimedOut {
            capability: capability.to_string(),
        };
    }
    if let Some(what) = not_found_what
        && !errors.is_empty()
        && errors
            .iter()
            .all(|e| matches!(e, BorsaError::NotFound { .. }))
    {
        return BorsaError::not_found(what);
    }
    BorsaError::AllProvidersFailed(errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn collapse_errors_all_timeouts() {
        let errors = vec![
            BorsaError::provider_timeout("p1", "quote"),
            BorsaError::provider_timeout("p2", "quote"),
        ];
        let e = collapse_errors(
            Capability::Quote,
            true,
            errors,
            Some("quote for AAPL".to_string()),
        );
        match e {
            BorsaError::AllProvidersTimedOut { capability } => {
                assert_eq!(capability, Capability::Quote.to_string());
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[tokio::test]
    async fn collapse_errors_all_not_found() {
        let errors = vec![BorsaError::not_found("x"), BorsaError::not_found("y")];
        let e = collapse_errors(
            Capability::Quote,
            true,
            errors,
            Some("quote for AAPL".to_string()),
        );
        match e {
            BorsaError::NotFound { what } => assert_eq!(what, "quote for AAPL"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[tokio::test]
    async fn collapse_errors_unsupported_when_no_attempts() {
        let e = collapse_errors(
            Capability::Quote,
            false,
            vec![],
            Some("quote for AAPL".to_string()),
        );
        match e {
            BorsaError::Unsupported { capability } => {
                assert_eq!(capability, Capability::Quote.to_string());
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[tokio::test]
    async fn collapse_errors_mixed_maps_to_all_failed() {
        let errors = vec![BorsaError::not_found("x"), BorsaError::Other("oops".into())];
        let e = collapse_errors(
            Capability::Quote,
            true,
            errors.clone(),
            Some("quote for AAPL".to_string()),
        );
        match e {
            BorsaError::AllProvidersFailed(es) => assert_eq!(es.len(), errors.len()),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[tokio::test]
    async fn join_with_deadline_times_out() {
        use std::time::Duration;
        let tasks = vec![async {
            tokio::time::sleep(Duration::from_millis(50)).await;
            1
        }];
        let res = join_with_deadline(tasks, Some(Duration::from_millis(1))).await;
        assert!(matches!(res, Err(BorsaError::RequestTimeout { .. })));
    }
}

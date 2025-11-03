use borsa_core::{BorsaError, Capability};

/// Join a collection of tasks and apply an optional request-level deadline.
///
/// This wraps `futures::future::join_all(tasks)` with `crate::core::with_request_deadline`.
/// On timeout, the inner helper returns `BorsaError::RequestTimeout("request")` which
/// call sites can remap to a more specific capability label as needed.
///
/// # Errors
/// Returns `BorsaError::RequestTimeout` if the provided `deadline` elapses before
/// all tasks complete.
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
#[must_use]
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

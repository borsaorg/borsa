use borsa_core::BorsaError;

pub fn collapse_stream_errors(errors: Vec<BorsaError>) -> BorsaError {
    let mut actionable: Vec<BorsaError> = errors
        .into_iter()
        .flat_map(borsa_core::BorsaError::flatten)
        .filter(borsa_core::BorsaError::is_actionable)
        .collect();
    match actionable.len() {
        0 => BorsaError::unsupported(borsa_core::Capability::StreamQuotes.to_string()),
        1 => actionable.remove(0),
        _ => BorsaError::AllProvidersFailed(actionable),
    }
}

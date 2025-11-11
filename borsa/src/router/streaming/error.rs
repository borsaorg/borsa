use borsa_core::{BorsaError, Capability};

pub fn collapse_stream_errors(capability: Capability, errors: Vec<BorsaError>) -> BorsaError {
    let mut actionable: Vec<BorsaError> = errors
        .into_iter()
        .flat_map(borsa_core::BorsaError::flatten)
        .filter(borsa_core::BorsaError::is_actionable)
        .collect();
    match actionable.len() {
        0 => BorsaError::unsupported(capability.to_string()),
        1 => actionable.remove(0),
        _ => BorsaError::AllProvidersFailed(actionable),
    }
}

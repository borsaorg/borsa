use borsa::{BorsaError, Capability, collapse_errors, join_with_deadline};

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

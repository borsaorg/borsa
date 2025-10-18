use borsa_core::stream::StreamHandle;

#[tokio::test(flavor = "multi_thread")]
async fn streamhandle_stop_graceful() {
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let (done_tx, done_rx) = tokio::sync::oneshot::channel::<()>();

    let task = tokio::spawn(async move {
        // Wait for stop signal, then signal completion
        let _ = stop_rx.await;
        let _ = done_tx.send(());
    });

    let handle = StreamHandle::new(task, stop_tx);
    handle.stop().await; // should await task completion

    // Verify the task completed due to graceful stop, not abort
    let _ = tokio::time::timeout(std::time::Duration::from_millis(100), done_rx)
        .await
        .expect("task did not complete after stop()");
}

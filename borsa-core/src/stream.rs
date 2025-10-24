use tokio::sync::oneshot;
use tokio::task::JoinHandle;

/// Abstraction over a handle that can be queried for completion and aborted.
pub trait Abortable {
    /// Abort the underlying task if it is still running.
    fn abort(&mut self);
    /// Return `true` if the underlying task has completed.
    fn is_finished(&self) -> bool;
}

impl Abortable for JoinHandle<()> {
    fn abort(&mut self) {
        // JoinHandle::abort takes &self
        Self::abort(self);
    }

    fn is_finished(&self) -> bool {
        Self::is_finished(self)
    }
}

/// Abstraction over a one-shot stop signal.
pub trait Stoppable {
    /// Send a best-effort stop signal to request graceful shutdown.
    fn send(self);
}

impl Stoppable for oneshot::Sender<()> {
    fn send(self) {
        let _ = Self::send(self, ());
    }
}

/// Drop-time logic for stream handles:
/// - send a best-effort stop signal if present
/// - abort the task if it hasn't finished yet
pub fn drop_impl<H, S>(inner: &mut Option<H>, stop_tx: &mut Option<S>)
where
    H: Abortable,
    S: Stoppable,
{
    if let Some(tx) = stop_tx.take() {
        tx.send();
    }
    if let Some(mut h) = inner.take()
        && !h.is_finished()
    {
        h.abort();
    }
}

/// Minimal stream handle abstraction for long-lived streaming tasks.
///
/// Lifecycle contract:
/// - Prefer calling [`stop`](StreamHandle::stop) to request a graceful shutdown and await completion.
/// - Call [`abort`](StreamHandle::abort) for immediate, non-graceful termination.
/// - If dropped without an explicit shutdown, a best-effort stop signal is sent (if available) and
///   the underlying task is then aborted. The task may not observe the stop signal before abort.
#[derive(Debug)]
pub struct StreamHandle {
    inner: Option<JoinHandle<()>>,
    stop_tx: Option<oneshot::Sender<()>>,
}

impl StreamHandle {
    /// Create a new `StreamHandle`.
    ///
    /// Parameters:
    /// - `inner`: the spawned task driving the stream.
    /// - `stop_tx`: a one-shot used to request a graceful stop.
    ///
    /// Returns a handle that can be used to stop or abort the stream.
    #[must_use]
    pub const fn new(inner: JoinHandle<()>, stop_tx: oneshot::Sender<()>) -> Self {
        Self {
            inner: Some(inner),
            stop_tx: Some(stop_tx),
        }
    }

    /// Create a `StreamHandle` that can only abort the task (no graceful stop).
    ///
    /// This constructor is intended for connectors that do not support a
    /// cooperative shutdown signal. Dropping the handle (or calling
    /// [`abort`](Self::abort)) will force-cancel the underlying task.
    #[must_use]
    pub const fn new_abort_only(inner: JoinHandle<()>) -> Self {
        Self {
            inner: Some(inner),
            stop_tx: None,
        }
    }

    /// Gracefully stop the underlying stream task and await its completion.
    ///
    /// Sends a stop signal if available, then awaits the task. Any errors
    /// from the task are ignored.
    pub async fn stop(mut self) {
        if let Some(tx) = self.stop_tx.take() {
            let _ = tx.send(());
        }
        if let Some(inner) = self.inner.take() {
            let _ = inner.await;
        }
    }

    /// Force-abort the underlying stream task without waiting for completion.
    ///
    /// Prefer [`stop`](Self::stop) when possible to allow cleanup.
    pub fn abort(mut self) {
        if let Some(inner) = self.inner.take() {
            inner.abort();
        }
    }
}

impl Drop for StreamHandle {
    fn drop(&mut self) {
        crate::stream::drop_impl(&mut self.inner, &mut self.stop_tx);
    }
}

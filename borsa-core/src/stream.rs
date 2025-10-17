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

impl Stoppable for tokio::sync::oneshot::Sender<()> {
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

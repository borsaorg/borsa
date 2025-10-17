use borsa_core::stream::{Abortable, Stoppable, drop_impl};

// Loom model types implementing Abortable/Stoppable using loom primitives
mod model {
    use super::*;
    use loom::sync::Arc;
    use loom::sync::atomic::{AtomicBool, Ordering};

    #[derive(Clone)]
    pub struct Handle {
        finished: Arc<AtomicBool>,
        aborted: Arc<AtomicBool>,
    }

    impl Handle {
        pub fn new() -> (Self, Arc<AtomicBool>, Arc<AtomicBool>) {
            let finished = Arc::new(AtomicBool::new(false));
            let aborted = Arc::new(AtomicBool::new(false));
            (
                Self {
                    finished: finished.clone(),
                    aborted: aborted.clone(),
                },
                finished,
                aborted,
            )
        }
        pub fn mark_finished(&self) {
            self.finished.store(true, Ordering::SeqCst);
        }
    }

    impl Abortable for Handle {
        fn abort(&mut self) {
            self.aborted.store(true, Ordering::SeqCst);
        }
        fn is_finished(&self) -> bool {
            self.finished.load(Ordering::SeqCst)
        }
    }

    #[derive(Clone)]
    pub struct StopTx {
        sent: Arc<AtomicBool>,
    }

    impl StopTx {
        pub fn new() -> (Self, Arc<AtomicBool>) {
            let sent = Arc::new(AtomicBool::new(false));
            (Self { sent: sent.clone() }, sent)
        }
    }

    impl Stoppable for StopTx {
        fn send(self) {
            self.sent.store(true, Ordering::SeqCst);
        }
    }
}

#[test]
fn drop_sends_stop_and_aborts_if_not_finished() {
    loom::model(|| {
        use model::*;

        // Create model handle and stop sender
        let (h, finished, aborted) = Handle::new();
        let (tx, sent) = StopTx::new();

        // Interleavings: the handle may finish before or after drop_impl runs.
        // Spawn a thread that marks finished; scheduler decides the ordering.
        let h2 = h.clone();
        loom::thread::spawn(move || {
            h2.mark_finished();
        });

        let mut inner = Some(h);
        let mut stop = Some(tx);
        drop_impl(&mut inner, &mut stop);

        // Stop should be sent regardless
        assert!(sent.load(loom::sync::atomic::Ordering::SeqCst));

        // If not finished at drop, task must have been aborted
        if !finished.load(loom::sync::atomic::Ordering::SeqCst) {
            assert!(aborted.load(loom::sync::atomic::Ordering::SeqCst));
        }
    });
}

#[test]
fn drop_sends_stop_and_does_not_abort_if_already_finished() {
    loom::model(|| {
        use model::*;

        let (h, _finished, aborted) = Handle::new();
        let (tx, sent) = StopTx::new();

        // Mark finished before drop_impl executes
        h.mark_finished();

        let mut inner = Some(h);
        let mut stop = Some(tx);
        drop_impl(&mut inner, &mut stop);

        assert!(sent.load(loom::sync::atomic::Ordering::SeqCst));
        assert!(!aborted.load(loom::sync::atomic::Ordering::SeqCst));
    });
}

#[test]
fn drop_abort_only_handle_aborts_without_stop() {
    loom::model(|| {
        use model::*;

        let (h, _finished, aborted) = Handle::new();
        let mut inner = Some(h);
        let mut stop: Option<model::StopTx> = None;
        drop_impl(&mut inner, &mut stop);

        assert!(aborted.load(loom::sync::atomic::Ordering::SeqCst));
    });
}

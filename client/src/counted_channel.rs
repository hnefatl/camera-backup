use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};

use anyhow::{anyhow, bail, ensure};

pub struct Sender<T> {
    sender: mpsc::SyncSender<T>,
    count: Rc<Mutex<usize>>,
    finished: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
}
impl<T: std::fmt::Debug> Sender<T> {
    pub fn send(&self, t: T) -> anyhow::Result<()> {
        // Terminate early if cancelled.
        ensure!(!self.cancelled.load(Ordering::SeqCst), "sender cancelled");

        self.sender.send(t).map_err(|e| anyhow!("failed to send: {:?}", e))?;
        let mut counter = self.count.lock().unwrap();
        *counter += 1;
        Ok(())
    }
    pub fn finish(self) {
        self.finished.store(true, Ordering::SeqCst);
    }
}
unsafe impl<T: Send> Send for Sender<T> {}

pub struct Receiver<T> {
    receiver: mpsc::Receiver<T>,
    count: Rc<Mutex<usize>>,
    finished: Arc<AtomicBool>,
    cancelled: Arc<AtomicBool>,
}
impl<T> Receiver<T> {
    pub fn recv(&self) -> anyhow::Result<Option<T>> {
        if let Ok(t) = self.receiver.recv() {
            // Terminate early if cancelled.
            ensure!(!self.cancelled.load(Ordering::SeqCst), "receiver cancelled");

            let mut counter = self.count.lock().unwrap();
            *counter -= 1;
            Ok(Some(t))
        } else if self.finished.load(Ordering::SeqCst) {
            // A RecvError after the sender's marked itself finished is okay, signal no items left.
            Ok(None)
        } else {
            bail!("Receiver closed")
        }
    }
    /// Get the number of items currently in the queue. This has no sync guarantees.
    pub fn len(&self) -> usize {
        *self.count.lock().unwrap()
    }
}
unsafe impl<T: Send> Send for Receiver<T> {}

pub fn channel<T>(bound: usize) -> (Sender<T>, Receiver<T>, impl Fn()) {
    let (sender, receiver) = mpsc::sync_channel(bound);
    let count = Rc::new(Mutex::new(0));
    let finished = Arc::new(AtomicBool::new(false));
    let cancelled = Arc::new(AtomicBool::new(false));
    (
        Sender {
            sender,
            count: count.clone(),
            finished: finished.clone(),
            cancelled: cancelled.clone(),
        },
        Receiver {
            receiver,
            count: count.clone(),
            finished: finished.clone(),
            cancelled: cancelled.clone(),
        },
        move || cancelled.store(true, Ordering::SeqCst),
    )
}

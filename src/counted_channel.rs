use std::rc::Rc;
use std::sync::{Mutex, mpsc};

pub struct Sender<T> {
    sender: mpsc::SyncSender<T>,
    count: Rc<Mutex<usize>>,
}
impl<T> Sender<T> {
    pub fn send(&self, t: T) -> Result<(), mpsc::SendError<T>> {
        self.sender.send(t)?;
        let mut counter = self.count.lock().unwrap();
        *counter += 1;
        Ok(())
    }
}
unsafe impl<T: Send> Send for Sender<T> {}

pub struct Receiver<T> {
    receiver: mpsc::Receiver<T>,
    count: Rc<Mutex<usize>>,
}
impl<T> Receiver<T> {
    pub fn recv(&self) -> Result<T, std::sync::mpsc::RecvError> {
        let r = self.receiver.recv()?;
        let mut counter = self.count.lock().unwrap();
        *counter -= 1;
        Ok(r)
    }
    /// Get the number of items currently in the queue. This has no sync guarantees.
    pub fn len(&self) -> usize {
        *self.count.lock().unwrap()
    }
}
unsafe impl<T: Send> Send for Receiver<T> {}

pub fn channel<T>(bound: usize) -> (Sender<T>, Receiver<T>) {
    let (sender, receiver) = mpsc::sync_channel(bound);
    let count = Rc::new(Mutex::new(0));
    (
        Sender {
            sender,
            count: count.clone(),
        },
        Receiver {
            receiver,
            count: count.clone(),
        },
    )
}

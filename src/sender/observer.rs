use std::sync::{Arc, RwLock};

/// File State Changed event
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct FileInfo {
    /// Object TOI
    pub toi: u128,
}

/// Event
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum Event {
    /// Transfer is started
    StartTransfer(FileInfo),
    /// Transfer has stopped
    StopTransfer(FileInfo),
}

/// Subscribe to events
pub trait Subscriber: Send + Sync {
    /// Flute sender event
    fn on_sender_event(&self, evt: &Event, now: std::time::SystemTime);
}

#[derive(Clone)]
pub struct ObserverList(Arc<RwLock<Vec<Arc<dyn Subscriber>>>>);

impl ObserverList {
    pub fn new() -> Self {
        ObserverList(Arc::new(RwLock::new(Vec::new())))
    }

    pub fn subscribe(&mut self, s: Arc<dyn Subscriber>) {
        self.0.write().unwrap().push(s);
    }

    pub fn unsubscribe(&mut self, s: Arc<dyn Subscriber>) {
        self.0
            .write()
            .unwrap()
            .retain(|a| !std::ptr::eq(a.as_ref() as *const _, s.as_ref() as *const _))
    }

    pub fn dispatch(&self, event: &Event, now: std::time::SystemTime) {
        let lock = self.0.read().unwrap();

        for subscriber in lock.iter() {
            subscriber.on_sender_event(event, now);
        }
    }
}

impl std::fmt::Debug for ObserverList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ObserverList")
    }
}

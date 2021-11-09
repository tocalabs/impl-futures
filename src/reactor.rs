use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::task::Waker;

use tokio::sync::mpsc::Receiver;

#[derive(Debug, Clone)]
pub enum EventType {
    Activity { activity_id: String },
    LockBots(String),
    UnlockBots(String),
}

#[derive(Debug, Clone)]
pub struct Event {
    waker: Option<Waker>,
    kind: EventType,
}

impl Event {
    pub fn new(waker: Option<Waker>, kind: EventType) -> Self {
        Event { waker, kind }
    }
}

pub struct Reactor {
    events_rx: Receiver<Event>,
    pub events: Arc<Mutex<HashMap<String, Event>>>,
}

impl Reactor {
    pub fn new(rx: Receiver<Event>) -> Self {
        Reactor {
            events_rx: rx,
            events: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn run(&mut self) {
        while let Some(event) = self.events_rx.recv().await {
            register_event(self, event).await;
        }
    }
}

async fn register_event(reactor: &mut Reactor, event: Event) {
    let cloned_waker = event.waker.clone();
    {
        match reactor.events.lock() {
            Ok(ref mut lock) => {
                lock.insert("".to_string(), event);
            }
            Err(_) => {
                panic!("Poisoned Mutex! - Failing");
            }
        }
    }

    if let Some(waker) = cloned_waker {
        waker.wake()
    }
}

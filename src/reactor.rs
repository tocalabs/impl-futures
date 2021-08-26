use std::collections::HashMap;
use std::sync::Arc;
use std::task::Waker;
use tokio::sync::mpsc::UnboundedReceiver as Receiver;
use tokio::sync::Mutex;

use rand::Rng;
use std::time::Duration;

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
    events: Arc<Mutex<HashMap<String, Event>>>,
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
    // let mut rng = rand::rngs::OsRng::default();
    // let rand_delay = rng.gen_range(0..=10);
    let cloned_waker = event.waker.clone();
    {
        let mut lock = reactor.events.lock().await;
        lock.insert("".to_string(), event);
    }
    if let Some(waker) = cloned_waker {
        //tokio::time::sleep(Duration::from_secs(rand_delay as u64)).await;
        waker.wake()
    }
}

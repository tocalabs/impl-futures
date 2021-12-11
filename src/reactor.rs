use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::task::Waker;

use tokio::sync::mpsc::Receiver;

#[derive(Debug, Clone)]
pub enum Event {
    Activity {
        node_id: String,
        activity_id: String,
        waker: Waker,
    },
}

pub struct Reactor {
    pub events: Arc<Mutex<HashMap<String, Waker>>>,
}

impl Reactor {
    pub fn new() -> Self {
        Reactor {
            events: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn run(self, mut internal_rx: Receiver<Event>) {
        let event_collection = self.events.clone();
        let internal_handle = tokio::task::spawn(async move {
            while let Some(event) = internal_rx.recv().await {
                register_event(event_collection.clone(), event).await;
            }
        });
        // External handle is for waiting on incoming msgs from external sources
        // Todo: Not used yet
        let external_handle = tokio::task::spawn(async move {});
        let _ = (internal_handle.await, external_handle.await);
    }
}

async fn register_event(event_collection: Arc<Mutex<HashMap<String, Waker>>>, event: Event) {
    match event {
        Event::Activity {
            node_id,
            activity_id,
            waker,
        } => {
            {
                let mut inner = event_collection.lock().expect("Locking failed");
                inner.insert(node_id, waker.clone());
            }
            // Change this waker.wake() to be a NATs send msg to Bot for activity execution
            // Get Activity
            // Execute Activity
            waker.wake();
        }
    }
}

async fn receive_event() {}

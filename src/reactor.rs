use std::collections::HashMap;
use std::iter::Iterator;
use std::sync::{Arc, Mutex};
use std::task::Waker;

use tokio::sync::mpsc::UnboundedReceiver as Receiver;

enum Event {
    LockBots,
    UnlockBots,
    ExecuteActivity,
    Cancel,
}

struct WakerEvent {
    event: Event,
    waker: Waker,
}

struct Reactor {
    rx: Receiver<WakerEvent>,
    inner: Arc<Mutex<HashMap<String, WakerEvent>>>,
}

struct WorkflowNode {
    id: i16,
    pointers: Pointer,
}

struct Pointer {
    pointers: Vec<String>,
}

struct Workflow {
    nodes: Vec<WorkflowNode>,
}

impl Iterator for Workflow {
    type Item = WorkflowNode;

    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}

trait Transport {
    fn send();
    fn fire_forget();
    fn receive();
}

struct Comms;

impl Reactor {
    fn new(rx: Receiver<WakerEvent>) -> Self {
        Reactor {
            rx,
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    async fn get_wakers(&mut self) {
        while let Some(message) = self.rx.recv().await {
            match self.inner.lock() {
                Ok(ref mut inner_mut) => inner_mut.insert("123".to_string(), message),
                Err(err) => panic!("Poisoned Mutex, {}", err),
            };
        }
    }
}

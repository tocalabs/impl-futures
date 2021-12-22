use futures::StreamExt;
use serde_json::from_str;
use std::collections::HashMap;
use std::str::from_utf8;
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

    pub async fn run(self, mut internal_rx: Receiver<Event>) -> Result<(), std::io::Error> {
        let nats_client = tokio_nats::Nats::connect("127.0.0.1:4222").await?;
        let mut response_sub = nats_client.subscribe("activity.response").await?;
        let event_collection = self.events.clone();
        let response_collection = self.events.clone();
        let client_clone = nats_client.clone();
        let internal_handle = tokio::task::spawn(async move {
            while let Some(event) = internal_rx.recv().await {
                let _ = register_event(event_collection.clone(), event, &client_clone).await;
            }
        });

        let external_handle = tokio::task::spawn(async move {
            while let Some(msg) = response_sub.next().await {
                let move_msg = msg;
                let node_id: String =
                    from_str::<String>(from_utf8(&move_msg.payload).expect("Unable to read msg"))
                        .expect("Unable to deserialize to string")
                        .replace(r#"""#, ""); //Not sure why I have to do this - todo: investigate
                let mut inner = response_collection.lock().expect("Locking failed");
                if let Some(waker) = inner.remove(&node_id) {
                    waker.wake();
                }
            }
        });
        let _ = (internal_handle.await, external_handle.await);
        Ok(())
    }
}

async fn register_event(
    event_collection: Arc<Mutex<HashMap<String, Waker>>>,
    event: Event,
    nats_client: &tokio_nats::Nats,
) -> Result<(), std::io::Error> {
    match event {
        Event::Activity {
            node_id,
            activity_id,
            waker,
        } => {
            {
                let mut inner = event_collection.lock().expect("Locking failed");
                inner.insert(node_id.clone(), waker.clone());
            }
            // Change this waker.wake() to be a NATs send msg to Bot for activity execution
            // Get Activity
            // Execute Activity
            let _ = nats_client
                .publish(tokio_nats::Msg::builder("activity.execute").json(&node_id)?)
                .await?;

            //waker.wake();
        }
    }
    Ok(())
}

async fn receive_event() {}

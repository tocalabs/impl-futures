//! Register interest in events and wake futures when the event occurs
//!
//!
//! ## Reactor
//! The Reactor stores a register of all the events that are currently being listened for.
//!
//! This has been designed so that it is completed decoupled from the main Execution Engine logic
//! meaning that you could extract the Reactor to a separate process entirely.
//! By separating this to another process you could update/patch the main EE component and the
//! reactor would store any events that came in and then we could resume jobs despite the core EE
//! component being swapped out. Of course this will require the interface between the EE and Reactor
//! remained consistent.
//!
//!
//! ## Events
//! An event is defined as an external message coming in notifying the EE that something has occurred.
//! This could be something like:
//! - The Bot is locked and unable to make progress
//! - An activity response has come in

use futures::StreamExt;
use serde_json::from_str;
use std::collections::HashMap;
use std::str::from_utf8;
use std::sync::{Arc, Mutex};
use std::task::Waker;

use tokio::sync::mpsc::Receiver;

/// A sum type/algebraic data type containing all the different types of Event that could occur
#[derive(Debug, Clone)]
pub enum Event {
    /// Defines the fields needed to execute an activity
    Activity {
        node_id: String,
        activity_id: String,
        waker: Waker,
    },
}

/// The Reactor struct stores event references for the events currently being waited on
///
/// This collection is safe to access across thread boundaries as we have wrapped with an [`Arc`](std::sync::Arc)
/// to satisfy borrowing the value across threads and it is also wrapped in a [`Mutex`](std::sync::Mutex)
/// to ensure that only one thread can write to it at a time
///
/// ---
/// Safety: We must make sure when a job is prematurely cancelled we drop any events being waited on
/// We could do this by implementing the drop trait on the [`Job`](crate::workflow::Job)
pub struct Reactor {
    /// A dictionary of events where the key is a unique identifier to the event
    /// and the value contains a `struct` with a mechanism to resume the `future`
    pub events: Arc<Mutex<HashMap<String, Waker>>>,
}

impl Reactor {
    /// Create a new reactor with an empty events register
    pub fn new() -> Self {
        Reactor {
            events: Default::default(),
        }
    }

    /// Connect to the external message broker to listen for events and react to them
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
                        .expect("Unable to deserialize to string");
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
            activity_id: _activity_id,
            waker,
        } => {
            {
                let mut inner = event_collection.lock().expect("Locking failed");
                inner.insert(node_id.clone(), waker.clone());
            }
            let _ = nats_client
                .publish(tokio_nats::Msg::builder("activity.execute").json(&node_id)?)
                .await?;
        }
    }
    Ok(())
}

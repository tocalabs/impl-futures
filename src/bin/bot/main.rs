//! Bot to run activities
//!
//! Bot needs to be able to accept NATs messages
//! The bot will then run an activity for an arbitrary amount of time
//! Return message to Core to inform it that it has completed the activity

use std::io;

use rand;
use rand::Rng;

#[tokio::main]
async fn main() -> Result<(), io::Error> {
    let nats_client = nats::connect("127.0.0.1:4222")?;
    let activity_subscription = nats_client.subscribe("activity.execute")?;
    while let Some(msg) = activity_subscription.next() {
        let client_clone = nats_client.clone();
        tokio::task::spawn(async move {
            let delay_duration = rand::thread_rng().gen_range(1..=10);
            tokio::time::sleep(std::time::Duration::from_secs(delay_duration)).await;
            println!("Running Activity");
            let _ = client_clone
                .clone()
                .publish("activity.response", msg.data)
                .expect("Unable to send response");
        });
    }
    Ok(())
}

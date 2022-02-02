use futures::StreamExt;
use std::io;

/// Bot to run activities
///
/// Bot needs to be able to accept NATs messages
/// The bot will then run an activity for an arbitrary amount of time
/// Return message to Core to inform it that it has completed the activity
#[tokio::main]
async fn main() -> Result<(), io::Error> {
    let nats_client = tokio_nats::Nats::connect("127.0.0.1:4222").await?;
    let mut activity_subscription = nats_client.subscribe("activity.execute").await?;
    while let Some(msg) = activity_subscription.next().await {
        let client_clone = nats_client.clone();
        tokio::task::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            let _ = client_clone
                .clone()
                .publish(tokio_nats::Msg::builder("activity.response").bytes(msg.payload))
                .await
                .expect("Unable to send response");
        });
    }
    Ok(())
}

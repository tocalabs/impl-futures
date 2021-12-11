use std::io;
use std::time::Duration;

use tokio::task;

use crate::reactor::{Event, Reactor};

mod executor;
mod node;
mod reactor;
mod spawner;
mod workflow;

/// Todo: Add Bot, separate reactor to different binary, add exclusive logic, enum iterator return type

/// The EE should be split into 2 main parts
/// 1. Executor - This is responsible for driving the workflows to completion and should contain
///    all the objects required for each workflow to be executed, think of this as a runtime.
/// 2. Reactor - The reactor is responsible for notifying the executor when a future can make
///    progress, this is done via the Waker API.
///
/// When a workflow is sent to the EE, the flow should be as follows:
/// 1. Spawn a new task which will perform all of the work associated with executing a wf to
/// completion
/// 2. Deserialize the workflow into a Job, the Job type should describe the entity as accurately
/// as possible
/// 3. Drive the workflow forward, for now it uses an iterator to do so
///
/// When a workflow reaches a point where it cannot make progress (e.g. waiting for Bots to Lock or
/// waiting for an Activity to complete) it should yield execution using the underlying mechanics
/// of rust's async/await.
///
///
/// The Executor is responsible for running each job and also the ability to cancel each job
///
/// ```rs
/// while let Some(msg) = nats.subscriber("job.*").next().await {
///     match msg.topic {
///         "job.execute" => {}
///         "job.cancel" => {}
///     }
/// }
/// ```
#[tokio::main]
async fn main() -> Result<(), io::Error> {
    // Create reactor channel
    let (reactor_tx, reactor_rx) = tokio::sync::mpsc::channel::<Event>(20);
    // Create Reactor
    let reactor = Reactor::new();
    let reactor_handle = task::spawn(async move {
        reactor.run(reactor_rx).await;
    });
    // create spawner channel
    let (spawn_tx, spawn_rx) = spawner::spawner_channel();
    // Create Spawner
    let mut spawner = spawner::Spawner::new(reactor_tx.clone(), spawn_rx);
    let spawner_handle = task::spawn(async move {
        spawner
            .spawn_job()
            .await
            .expect("Something went critically wrong");
    });

    spawn_tx.send(spawner::execute_msg("workflow.json")).await;
    /*
    // Connect to NATs server
    let server = tokio_nats::Nats::connect("127.0.0.1:9123").await?;
    // Set up subscribers
    let mut execution_subscriber = server.subscribe("jobs.execute").await?;
    let mut cancellation_subscriber = server.subscribe("jobs.cancel").await?;
     */
    // tokio::time::sleep(Duration::from_secs(3)).await;
    // spawn_tx.send(spawner::cancel_msg("workflow.json")).await;
    // Await the handles to reactor and spawner to make sure all tasks run to completion
    reactor_handle.await;
    spawner_handle.await;
    Ok(())
}

#[cfg(test)]
mod exclusive_tests {
    #[test]
    fn test_eval() {
        use eval::{to_value, Expr};
        let expression = "(foo == bar) || (spam != eggs)";
        let evaluation = Expr::new(expression)
            .value("foo", "hi")
            .value("bar", "ho")
            .value("spam", 4)
            .value("eggs", 45)
            .exec();
        //         assert_eq!(evaluation, Ok(to_value(false)));
        assert_eq!(evaluation, Ok(to_value(true)));
    }
}

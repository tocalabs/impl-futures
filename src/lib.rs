//! Execution Engine MkII
//!
//! ---
//!
//! The EE is split into 2 main parts
//! 1. [Executor](crate::executor) - This is responsible for driving the workflows to completion and should contain
//!    all the objects required for each workflow to be executed, think of this as a runtime.
//! 2. [Reactor](crate::reactor) - The reactor is responsible for notifying the executor when a future can make
//!    progress, this is done via the Waker API.
//!
//! When a workflow is sent to the EE, the flow is as follows:
//! 1. Spawn a new task which will perform all of the work associated with executing a wf to
//! completion
//! 2. Deserialize the workflow into a Job, the Job type should describe the entity as accurately
//! as possible
//! 3. Drive the workflow forward, this uses an event based stream to do so
//!
//! When a workflow reaches a point where it cannot make progress (e.g. waiting for Bots to Lock or
//! waiting for an Activity to complete) it should yield execution using the underlying mechanics
//! of Rust's async/await.
//!
//!
pub(crate) use std::io;

pub(crate) use tokio::task;

use crate::reactor::{Event, Reactor};
pub mod executor;
pub mod node;
pub mod reactor;
pub mod spawner;
pub mod workflow;

/// Sets up the runtime and starts the message broker listeners
///
/// The `start` function creates the structures used by the Runtime and starts the Reactor, Spawner and MsgBroker consumers.
///
/// 1. The [Reactor](crate::reactor) - This includes the channel to send messages to the reactor and
/// the Reactor struct itself. The Reactor is then run in a separate task and we `await` on the task's `Handle`.
/// 2. The [Spawner](crate::spawner) - First, a channel to communicate to the Spawner is created, then
/// a new Spawner struct is created, passing in the `Receiver` for the channel. The Spawner is run in
/// a background task and we `await` on the task's `Handle`.
/// 3. The [NATs](https://docs.nats.io/) consumer - This creates a NATs consumer listening for messages
/// on the `jobs.execute` and `jobs.cancel` topics.
pub async fn start() -> Result<(), io::Error> {
    // Create reactor channel
    let (reactor_tx, reactor_rx) = tokio::sync::mpsc::channel::<Event>(20);
    // Create Reactor
    let reactor = Reactor::new();
    let reactor_handle = task::spawn(async move {
        let _ = reactor.run(reactor_rx).await;
    });
    // create spawner channel
    let (spawn_tx, spawn_rx) = spawner::spawner_channel();
    // Create Spawner
    let mut spawner = spawner::Spawner::new(reactor_tx.clone(), spawn_rx);
    let spawner_handle = task::spawn(async move {
        spawner
            .run()
            .await
            .expect("Something went critically wrong");
    });

    /*

    let loop_no = 10;
    let start = std::time::Instant::now();

    for _ in 0..loop_no {
        let _ = spawn_tx.send(spawner::execute_msg("workflow.json")).await;
    }
    println!(
        "{:#?}------------------#################",
        start.elapsed() / loop_no / 3
    );
    println!(
        "{:#?}------------------#################",
        start.elapsed() / loop_no
    );
    println!("{:#?}------------------#################", start.elapsed());
    */

    // Connect to NATs server
    let server = nats::connect("127.0.0.1:4222")?;
    // Set up subscribers
    let mut _execution_subscriber = server.subscribe("jobs.execute")?;
    let mut _cancellation_subscriber = server.subscribe("jobs.cancel")?;

    // let _ = spawn_tx.send(spawner::execute_msg(&uuid::Uuid::new_v4().to_string())).await;
    // tokio::time::sleep(Duration::from_secs(10)).await;
    // let _ = spawn_tx.send(spawner::execute_msg(&uuid::Uuid::new_v4().to_string())).await;
    for _ in 0..1 {
        //tokio::time::sleep(Duration::from_millis(1000)).await;
        println!("Spawned");
        spawn_tx
            .send(spawner::execute_msg(&uuid::Uuid::new_v4().to_string()))
            .await;
    }

    //spawn_tx.send(spawner::cancel_msg("workflow.json")).await;
    // Await the handles to reactor and spawner to make sure all tasks run to completion
    let _ = (reactor_handle.await, spawner_handle.await);
    Ok(())
}

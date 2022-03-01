//! The logic for executing a job
//!
//! When a job is being executed, logically it is separated into two different areas:
//! 1. retrieving the next node to be run
//! 2. running the current node(s)
//!
//! ## Get Next Node
//! The [`Job`](crate::workflow::Job) struct has a method defined on it called [`Job.next_node()`](crate::workflow::Job::next_node)
//! which will return the next node or nodes. This works in an event based pattern whereby each
//! node, on completion, will send a message to the `Job` struct informing it that the node has
//! completed.
//!
//! The [`Message`](crate::workflow::Message) that is sent returns a status to let the job know whether the node completed
//! successfully. On successful completion, the executor will call `job.next_node(msg.pointer)` to
//! fetch the next node(s) to be executed.
//!
//! ## Run the current node(s)
//! As each node is returned, it needs to be run. Every node implements the [`Node`](crate::node::Node) type
//! which means that to run it we can simply call the `run` method. Each node is run in a new task
//! so we can run multiple nodes in parallel. This is so we can accommodate for parallels and exclusives.
//!
//! ```
//! for node in job.next_node(...) {
//!     task::spawn(node.run().await) // executes in background, immediately returning
//! }
//! ```

use std::io;

use tokio::{fs, task};
use tokio::select;
use tokio::sync::{broadcast, mpsc::{Receiver, Sender}};
use tokio::time::Instant;

use crate::reactor::Event;
use crate::workflow;
use crate::workflow::{Job, Message, NodeStatus};

/// The execute_handler function takes a workflow ID, gets the workflow and creates a Job struct
/// from that workflow. Once we have a Job struct, the job is driven to completion by the [`run`](self::run) function.
pub async fn execute_handler(file: &str, rc_clone: Sender<Event>, cancellation_tx: broadcast::Sender<()>) -> Result<(), io::Error> {
    let start = Instant::now();
    let wf_json = fs::read_to_string(file).await?;
    let wf: workflow::Workflow = serde_json::from_str(&wf_json)?;
    let (job, job_rx) = Job::new(&wf, &rc_clone);
    println!("{:#?}", job);
    run(job, job_rx, cancellation_tx).await; // todo: return result here so we can bubble up the result
    println!("Time taken to run Job: {:#?}", start.elapsed());
    Ok(())
}

/// Take ownership of a job and the jobs receiver and drive the job to completion by getting
/// each node and running them as they can be run.
///
/// Each node is responsible for notifying the job that it can move forward
/// The next node function will need to take a pointer to the current node that has finished
/// So it knows where to resume the job from
async fn run(job: Job, mut rx: Receiver<Message>, cancellation_tx: broadcast::Sender<()>) {
    if let Some(next_node) = job.next_node(None) {
        // gets start node at very beginning
        let _ = next_node.get(0).expect("Missing Start Node").run().await; // Waiting for start node to complete
    }

    while let Some(msg) = rx.recv().await {
        match msg.status {
            NodeStatus::Failed => { /* The job must now return with status of failed */ }
            NodeStatus::Success => match job.next_node(Some(msg.pointer)) {
                Some(nodes) => {
                    for node in nodes {
                        let mut cancellation_sub = cancellation_tx.clone().subscribe();
                        let node_clone = node.clone();
                        task::spawn(async move {
                            select! {
                                _ = node_clone.run() => {},
                                _ = cancellation_sub.recv() => {
                                    println!("Node cancelled");
                                }
                            }
                        });
                    }
                }
                None => {
                    //drop(job);
                    break; // This is because we don't drop the Job when End is returned so we need to manually break out of this loop
                    // We could manually drop Job here which would have the same effect
                }
            },
        }
    }
    //todo!() // what is there to do here? I'm not sure?!?
}

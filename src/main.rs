use std::io;
mod node;
mod reactor;
mod workflow;

use crate::reactor::Reactor;
use crate::workflow::Job;
use futures::future::join_all;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::fs;

type Sender<T> = tokio::sync::mpsc::UnboundedSender<T>;

/// The EE should be split into 2 main parts
/// 1. Executor - This is responsible for driving the workflows to completion and should contain
///    all the objects required for each workflow to be executed, think of this as a runtime.
/// 2. Reactor - The reactor is responsible for notifying the executor when a future can make
///    progress, this is done via the Waker API.
///
/// When a workflow is sent to the EE, the flow should be as follows:
/// a) Spawn a new task which will perform all of the work associated with executing a wf to
/// completion
/// b) Deserialize the workflow into a Job, the Job type should describe the entity as accurately
/// as possible
/// c) Drive the workflow forward, for now it uses an iterator to do so
///
/// When a workflow reaches a point where it cannot make progress (e.g. waiting for Bots to Lock or
/// waiting for an Activity to complete) it should yield execution using the underlying mechanics
/// of rust's async/await.
///

/// The Executor is responsible for running each job and also the ability to cancel each job
///
/// ```
/// while let Some(msg) = nats.subscriber("job.*").next().await {
///     match msg.topic {
///         "job.execute" => {}
///         "job.cancel" => {}
///     }
/// }
/// ```
struct Executor<T> {
    reactor_channel: Sender<T>,
    jobs: HashMap<String, tokio::sync::oneshot::Sender<bool>>,
}

impl<T> Executor<T> {
    async fn spawn_job(&self, workflow_id: &str) -> Result<(), io::Error> {
        let wf_json = fs::read_to_string(workflow_id).await?;

        // select!(task::spawn -> job, cancellation token);
        Ok(())
    }
    async fn cancel_job(&mut self, job_id: &str) -> Result<(), io::Error> {
        if let Some(cancellation) = self.jobs.remove(job_id) {
            cancellation
                .send(true)
                .expect("Job was unable to be cancelled, Receiver dropped");
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), io::Error> {
    let wf_json = fs::read_to_string("workflow.json").await?;
    let wf: workflow::Workflow = serde_json::from_str(&wf_json)?;
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let arc_wf = Arc::new(wf);
    let mut inner_reactor: Reactor = reactor::Reactor::new(rx);
    let event_cp = inner_reactor.events.clone();
    let reactor_handle = tokio::task::spawn(async move { inner_reactor.run().await });
    let start = std::time::Instant::now();
    for _ in 1..=10000 {
        let wf = arc_wf.clone();
        let cloned_tx = tx.clone();
        tokio::spawn(async move {
            let job = Job::new(&wf, &cloned_tx);
            for element in job {
                element.run().await.unwrap();
            }
        });
    }

    println!("{:#?}", start.elapsed() / 10_000 / 3);
    drop(tx);
    let _ = reactor_handle.await;
    Ok(())
}

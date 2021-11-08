use std::io;
mod node;
mod reactor;
mod workflow;

use crate::reactor::{Event, Reactor};
use crate::workflow::Job;
use futures::StreamExt;
use std::collections::HashMap;
use std::str::from_utf8;
use std::sync::{Arc, Mutex};
use tokio::fs;
use tokio::select;
use tokio::task;
use tokio_nats;
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
/// ```rs
/// while let Some(msg) = nats.subscriber("job.*").next().await {
///     match msg.topic {
///         "job.execute" => {}
///         "job.cancel" => {}
///     }
/// }
/// ```
struct Executor {
    reactor_channel: Sender<Event>,
    jobs: HashMap<String, tokio::sync::oneshot::Sender<()>>,
}

impl Executor {
    fn new(tx: Sender<Event>) -> Self {
        Self {
            reactor_channel: tx,
            jobs: HashMap::new(),
        }
    }
    async fn spawn_job(
        job_store: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<()>>>>,
        reactor_channel: Sender<Event>,
        workflow_id: String,
    ) -> Result<(), io::Error> {
        let wf_json = fs::read_to_string(workflow_id).await?;
        let wf: workflow::Workflow = serde_json::from_str(&wf_json)?;
        let (canceller, received) = tokio::sync::oneshot::channel::<()>();
        let job_id = String::from("abc");
        {
            job_store
                .lock()
                .expect("Mutex broken")
                .insert(job_id, canceller);
        }
        let job_wrapper = async move {
            let job = Job::new(&wf, &reactor_channel);
            for element in job {
                element.run().await.unwrap();
            }
        };
        select!(
            _ = job_wrapper => {}
            _ = received => {}
        );
        Ok(())
    }
    async fn cancel_job(&mut self, job_id: &str) -> Result<(), io::Error> {
        if let Some(cancellation) = self.jobs.remove(job_id) {
            cancellation
                .send(())
                .expect("Job was unable to be cancelled, Receiver dropped");
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), io::Error> {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Event>();
    let mut executor = Executor::new(tx);
    let job_store = Arc::new(Mutex::new(executor.jobs));
    let server = tokio_nats::Nats::connect("127.0.0.1:9123").await?;
    let mut execution_subscriber = server.subscribe("jobs.execute").await?;
    let mut cancellation_subscriber = server.subscribe("jobs.cancel").await?;
    let execute_job_store = job_store.clone();
    let rc = executor.reactor_channel.clone();
    tokio::task::spawn(async move {
        while let Some(msg) = execution_subscriber.next().await {
            println!("woah");
            let wf_id = from_utf8(&msg.payload)
                .expect("Message should be able to be decoded to a string")
                .to_string();
            let wf_clone = wf_id.clone();
            let _ = task::spawn(Executor::spawn_job(
                execute_job_store.clone(),
                rc.clone(),
                wf_clone,
            ));
        }
    });
    Ok(())
}

use std::io;
mod node;
mod reactor;
mod workflow;

use crate::reactor::{Event, Reactor};
use crate::workflow::Job;
use futures::StreamExt;
use std::collections::HashMap;
use std::future::Future;
use std::str::from_utf8;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::fs;
use tokio::select;
use tokio::sync::RwLock;
use tokio::task;
use tokio_nats;

type Sender<T> = tokio::sync::mpsc::Sender<T>;

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
    async fn cancel_job(&mut self, job_id: &str) -> Result<(), io::Error> {
        if let Some(cancellation) = self.jobs.remove(job_id) {
            cancellation
                .send(())
                .expect("Job was unable to be cancelled, Receiver dropped");
        }
        Ok(())
    }
}

enum SpawnerMsg {
    Execute(String),
    Cancel(String),
}

/// Create Handlers
///
///
struct Spawner {
    reactor_channel: tokio::sync::mpsc::Sender<Event>,
    rx: tokio::sync::mpsc::Receiver<SpawnerMsg>,
    jobs: Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<()>>>>,
}

impl Spawner {
    fn new(
        rc: tokio::sync::mpsc::Sender<Event>,
        listener: tokio::sync::mpsc::Receiver<SpawnerMsg>,
    ) -> Self {
        Spawner {
            reactor_channel: rc,
            rx: listener,
            jobs: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    async fn spawn_job(&mut self) -> Result<(), io::Error> {
        while let Some(msg) = self.rx.recv().await {
            match msg {
                SpawnerMsg::Execute(file) => {
                    let cloned_jobs = self.jobs.clone();
                    let rc_clone = self.reactor_channel.clone();
                    let (cancellation_tx, cancellation_rx) = tokio::sync::oneshot::channel::<()>();
                    let mut write_jobs = cloned_jobs.lock().expect("Locking failed");
                    write_jobs.insert(file.clone(), cancellation_tx);
                    drop(write_jobs);
                    task::spawn(async move {
                        let execute_fut = async move {
                            println!("{}", file);
                            let wf_json = fs::read_to_string(file)
                                .await
                                .expect("Unable to read workflow file");
                            let wf: workflow::Workflow = serde_json::from_str(&wf_json)
                                .unwrap_or_else(|_| panic!("Unable to read JSON: {}", wf_json));
                            let job = Job::new(&wf, &rc_clone);
                            for element in job {
                                element.run().await.unwrap();
                                println!("Running Node");
                                tokio::time::sleep(Duration::from_secs(2)).await;
                            }
                        };
                        select!(
                            _ = execute_fut => {}
                            _ = cancellation_rx => {
                                println!("The job was cancelled");
                            }
                        );
                    });
                }
                SpawnerMsg::Cancel(job_id) => {
                    let cloned_jobs = self.jobs.clone();
                    let mut n = cloned_jobs.lock().expect("Failed to lock");
                    if let Some(sender) = n.remove(&job_id) {
                        sender.send(());
                    }
                }
            }
        }
        Ok(())
    }
}

async fn execute_handler(mut subscriber: tokio_nats::Subscription) -> Result<(), io::Error> {
    while let Some(msg) = subscriber.next().await {
        let wf_id = from_utf8(&msg.payload)
            .expect("Unable to decode message to a string")
            .to_string();
        let _ = task::spawn(async move {});
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), io::Error> {
    // Create reactor channel
    let (reactor_tx, reactor_rx) = tokio::sync::mpsc::channel::<Event>(20);
    // Create Reactor
    let mut reactor = Reactor::new(reactor_rx);
    let reactor_handle = task::spawn(async move {
        reactor.run().await;
    });
    // create spawner channel
    let (spawn_tx, spawn_rx) = tokio::sync::mpsc::channel::<SpawnerMsg>(20);
    // Create Spawner
    let mut spawner = Spawner::new(reactor_tx.clone(), spawn_rx);
    let spawner_handle = task::spawn(async move {
        spawner
            .spawn_job()
            .await
            .expect("Something went critically wrong");
    });

    spawn_tx
        .send(SpawnerMsg::Execute("workflow.json".to_string()))
        .await;
    /*
    // Connect to NATs server
    let server = tokio_nats::Nats::connect("127.0.0.1:9123").await?;
    // Set up subscribers
    let mut execution_subscriber = server.subscribe("jobs.execute").await?;
    let mut cancellation_subscriber = server.subscribe("jobs.cancel").await?;
     */
    tokio::time::sleep(Duration::from_secs(3)).await;
    spawn_tx
        .send(SpawnerMsg::Cancel("workflow.json".to_string()))
        .await;
    // Await the handles to reactor and spawner to make sure all tasks run to completion
    reactor_handle.await;
    spawner_handle.await;
    Ok(())
}

//! Spawns new jobs and handles cancellations
//!
//! The job of the Spawner is to receive either a message indicating to execute a workflow or cancel a job.
//! It executes each new job in a new `Task` allowing for parallelism so multiple jobs can be submitted to the
//! async executor at once.

use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex};

use tokio::select;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::oneshot;
use tokio::task;

use crate::executor;
use crate::reactor::Event;

/// Different types of message that can be received by the Spawner
pub enum SpawnerMsg {
    /// Execute contains the workflow ID to be executed
    Execute(String),
    /// Cancel contains the job ID which needs to be cancelled
    Cancel(String),
}

/// Create channel required for communication with the Spawner
pub fn spawner_channel() -> (Sender<SpawnerMsg>, Receiver<SpawnerMsg>) {
    tokio::sync::mpsc::channel::<SpawnerMsg>(20)
}

/// Creates `SpawnerMsg::Execute` variant from `&str`
pub fn execute_msg(wf_id: &str) -> SpawnerMsg {
    SpawnerMsg::Execute(wf_id.to_string())
}

/// Creates `SpawnerMsg::Cancel` variant from `&str`
pub fn cancel_msg(job_id: &str) -> SpawnerMsg {
    SpawnerMsg::Cancel(job_id.to_string())
}

/// The Spawner struct contains the state required to create and cancel jobs
pub struct Spawner {
    /// The sender half of the reactor channel, this is cloned for each new job
    reactor_channel: Sender<Event>,
    /// The receiver half for the Spawner to receive `SpawnerMsg`'s
    rx: Receiver<SpawnerMsg>,
    /// A thread safe register of job ids and their cancellation channel, to be used when the Spawner
    /// receives a `SpawnerMsg::Cancel` message
    jobs: Arc<Mutex<HashMap<String, oneshot::Sender<()>>>>,
}

impl Spawner {
    /// Create a new Spawner from the `Sender` half of the receiver channel and the `Receiver` half
    /// of the Spawner channel
    pub fn new(
        rc: tokio::sync::mpsc::Sender<Event>,
        listener: tokio::sync::mpsc::Receiver<SpawnerMsg>,
    ) -> Self {
        Spawner {
            reactor_channel: rc,
            rx: listener,
            jobs: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    /// Run the spawner
    ///
    /// When running, the spawner `await`s messages on `self.rx` and if the messages are:
    /// 1. [`SpawnerMsg::Execute`](SpawnerMsg::Execute) - it will create the cancellation channel for this job and run the executor
    /// for this job in a new `task`
    /// 2. [`SpawnerMsg::Cancel`](SpawnerMsg::Cancel) - it will broadcast the cancellation message to the appropriate job so
    /// that the job is aborted gracefully
    pub async fn run(&mut self) -> Result<(), io::Error> {
        while let Some(msg) = self.rx.recv().await {
            match msg {
                SpawnerMsg::Execute(file) => {
                    let cloned_jobs = self.jobs.clone();
                    let rc_clone = self.reactor_channel.clone();
                    let (cancellation_tx, cancellation_rx) = tokio::sync::oneshot::channel::<()>();
                    {
                        let mut write_jobs = cloned_jobs.lock().expect("Locking failed");
                        write_jobs.insert(file.clone(), cancellation_tx);
                    }
                    //executor::execute_handler(&file, rc_clone).await;
                    task::spawn(async move {
                        select!(
                            _ = executor::execute_handler(&file, rc_clone) => {
                                println!("The job has completed!")
                            }
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
                        sender.send(()).unwrap();
                    }
                }
            }
        }
        Ok(())
    }
}

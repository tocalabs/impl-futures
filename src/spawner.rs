use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex};

use tokio::select;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::oneshot;
use tokio::task;

use crate::executor;
use crate::reactor::Event;

pub enum SpawnerMsg {
    Execute(String),
    Cancel(String),
}

pub fn spawner_channel() -> (Sender<SpawnerMsg>, Receiver<SpawnerMsg>) {
    tokio::sync::mpsc::channel::<SpawnerMsg>(20)
}

pub fn execute_msg(wf_id: &str) -> SpawnerMsg {
    SpawnerMsg::Execute(wf_id.to_string())
}

pub fn cancel_msg(job_id: &str) -> SpawnerMsg {
    SpawnerMsg::Cancel(job_id.to_string())
}

pub struct Spawner {
    reactor_channel: Sender<Event>,
    rx: Receiver<SpawnerMsg>,
    jobs: Arc<Mutex<HashMap<String, oneshot::Sender<()>>>>,
}

impl Spawner {
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
    pub async fn spawn_job(&mut self) -> Result<(), io::Error> {
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

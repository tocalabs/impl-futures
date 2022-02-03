//! The Executor mod contains the code that will execute a workflow

use std::io;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::{fs, task};

use crate::reactor::Event;
use crate::workflow;
use crate::workflow::{Job, Message, NodeStatus};

/// The execute_handler function takes a workflow ID, gets the workflow and creates a Job struct
/// from that workflow. Once we have a Job struct, the job is driven to completion through an
/// iterator.
pub async fn execute_handler(file: &str, rc_clone: Sender<Event>) -> Result<(), io::Error> {
    let wf_json = fs::read_to_string(file).await?;
    let wf: workflow::Workflow = serde_json::from_str(&wf_json)?;
    let (job, job_rx) = Job::new(&wf, &rc_clone);
    //run_job(job).await?;
    run(job, job_rx).await;
    Ok(())
}

async fn run(job: Job, mut rx: Receiver<Message>) {
    //Each node is responsible for notifying the job that it can move forward
    //The next node function will need to take a pointer to the current node that has finished
    // So it knows where to resume the job from
    if let Some(next_node) = job.next_node(None) {
        // gets start node at very beginning
        next_node.get(0).expect("Missing Start Node").run().await; // Waiting for start node to complete
    }
    while let Some(msg) = rx.recv().await {
        match msg.status {
            NodeStatus::Failed => {}
            NodeStatus::Success => match job.next_node(Some(msg.pointer)) {
                Some(nodes) => {
                    for node in nodes {
                        task::spawn(async move { node.clone().run().await });
                    }
                }
                None => {}
            },
        }
    }
    todo!()
}

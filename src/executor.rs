//! The Executor mod contains the code that will execute a workflow
use std::error::Error;
use futures::future::join_all;
use futures::future::{BoxFuture, FutureExt};
use std::io;

use tokio::fs;
use tokio::sync::mpsc::Sender;

use crate::reactor::Event;
use crate::workflow;
use crate::workflow::Job;

/// The execute_handler function takes a workflow ID, gets the workflow and creates a Job struct
/// from that workflow. Once we have a Job struct, the job is driven to completion through an
/// iterator.
pub async fn execute_handler(file: &str, rc_clone: Sender<Event>) -> Result<(), io::Error> {
    let wf_json = fs::read_to_string(file).await?;
    let wf: workflow::Workflow = serde_json::from_str(&wf_json)?;
    let job = Job::new(&wf, &rc_clone);
    run_job(job).await?;
    Ok(())
}

async fn run(job: Job) -> Result<(), Box<dyn Error>> {
    let tx, rx = tokio::sync::mpsc::channel::<workflow::Message>(20);
    let job = Job::new(tx) //pass in tx for event based notification for when to progress
    //Each node is responsible for notifying the job that it can move forward
    //The next node function will need to take a pointer to the current node that has finished
    // So it knows where to resume the job from
    let next_node = job.next(None); // gets start node at very beginning
    next_node.run().await(); // Waiting for start node to complete
    while let Some(msg) = rx.recv().await {
        match msg.status {
            NodeStatus::Failed => { return Err(()) }
            NodeStatus::Success => {
                match job.next_nodes(msg) {
                    Some(nodes) => {}
                }
                let next_nodes = job.next_nodes(msg); //next_nodes could return multiple jobs
                if nodes.len() == 1 {
                    node.run().await
                }
                for node in next_nodes {
                    task::spawn(node.run())
                }
            }
        }


    }
    todo!()
}

fn run_job(job: Job) -> BoxFuture<'static, Result<(), io::Error>> {
    async move {
        for element in job {
            match element {
                workflow::NextNodes::Single(node) => {
                    node.execute().await.unwrap();
                }
                workflow::NextNodes::Multiple(parallel) => {
                    let mut tasks = vec![];
                    for path in parallel {
                        tasks.push(tokio::task::spawn(run_job(path)));
                    }
                    join_all(tasks);
                }
            }

            println!("Running Node");
        }
        Ok(())
    }
    .boxed()
}

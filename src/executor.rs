//! The Executor mod contains the code that will execute a workflow
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

fn run_job(job: Job) -> BoxFuture<'static, Result<(), io::Error>> {
    async move {
        for element in job {
            match element {
                workflow::NextNodes::Single(node) => {
                    node.run().await.unwrap();
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

use std::io;
mod node;
mod reactor;
mod workflow;

use crate::workflow::Job;
use futures::future::join_all;
use std::sync::Arc;
use tokio::fs;
use tokio::task::JoinHandle;

#[tokio::main]
async fn main() -> Result<(), io::Error> {
    let wf_json = fs::read_to_string("workflow.json").await?;
    let wf: workflow::Workflow = serde_json::from_str(&wf_json)?;
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let arc_wf = Arc::new(wf);
    //let mut tasks = Vec::with_capacity(100000);
    let start = std::time::Instant::now();
    let mut task: Option<JoinHandle<()>> = None;
    for _ in 0..1_000_000 {
        let wf = arc_wf.clone();
        let cloned_tx = tx.clone();
        task = Some(tokio::spawn(async move {
            let job = Job::new(&wf, &cloned_tx);
            for element in job {
                element.run().await.unwrap();
            }
        }));
    }
    //join_all(tasks).await;
    task.unwrap().await;
    println!("{:#?}", start.elapsed());

    Ok(())
}

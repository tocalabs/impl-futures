use std::io;
mod node;
mod reactor;
mod workflow;

use crate::reactor::Reactor;
use crate::workflow::Job;
use futures::future::join_all;
use std::sync::Arc;
use tokio::fs;

#[tokio::main]
async fn main() -> Result<(), io::Error> {
    let wf_json = fs::read_to_string("workflow.json").await?;
    let wf: workflow::Workflow = serde_json::from_str(&wf_json)?;
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    let arc_wf = Arc::new(wf);
    let mut tasks = Vec::with_capacity(10000);
    let mut inner_reactor: Reactor = reactor::Reactor::new(rx);
    let event_cp = inner_reactor.events.clone();
    let reactor_handle = tokio::task::spawn(async move { inner_reactor.run().await });
    let start = std::time::Instant::now();
    for _ in 1..=10000 {
        let wf = arc_wf.clone();
        let cloned_tx = tx.clone();
        tasks.push(tokio::spawn(async move {
            let job = Job::new(&wf, &cloned_tx);
            for element in job {
                element.run().await.unwrap();
            }
        }));
    }

    join_all(tasks).await;
    println!("{:#?}", start.elapsed());
    drop(tx);
    let _ = reactor_handle.await;

    Ok(())
}

use std::io;
use std::time::Duration;

use tokio::fs;
use tokio::sync::mpsc::Sender;

use crate::reactor::Event;
use crate::workflow;
use crate::workflow::Job;

pub async fn execute_handler(file: &str, rc_clone: Sender<Event>) -> Result<(), io::Error> {
    let wf_json = fs::read_to_string(file).await?;
    let wf: workflow::Workflow = serde_json::from_str(&wf_json)?;
    let job = Job::new(&wf, &rc_clone);
    for element in job {
        element.run().await.unwrap();
        println!("Running Node");
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    Ok(())
}

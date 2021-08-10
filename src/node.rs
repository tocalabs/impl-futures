use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::time::Duration;
use thiserror::Error;

enum NodeStatus {
    Failed,
    Success,
}

#[derive(Error, Debug)]
enum NodeError {
    #[error("Failed")]
    Failed,
    #[error("Comms")]
    Comms,
}

struct Start;
struct Activity;
struct Parallel;
struct Exclusive;
struct End;

trait Workflow {}

trait Pipeline {
    fn create_node(&self, node: &impl Node);
    fn update_node(&self, node: &impl Node);
    fn create_meta(wf: impl Workflow);
    fn update_meta(wf: impl Workflow);
}

pub trait Node {
    type Future: Future<Output = Result<Self::Item, NodeError>>;
    type Item;
    fn run(&self) -> Self::Future;
    fn add_pipeline(&self, pipeline: impl Pipeline)
    where
        Self: Sized,
    {
        pipeline.create_node(self);
    }
}

type NodeFuture<T> = Pin<Box<dyn Future<Output = Result<T, NodeError>>>>;

impl Node for Start {
    type Future = NodeFuture<Self::Item>;
    type Item = ();

    fn run(&self) -> Self::Future {
        Box::pin(async move { Ok(()) })
    }
}

impl Node for Parallel {
    type Future = NodeFuture<Self::Item>;
    type Item = ();

    fn run(&self) -> Self::Future {
        Box::pin(async move {
            tokio::time::sleep(Duration::from_secs(5)).await;
            Ok(())
        })
    }
}

enum TriggerType {
    Response,
    Fire,
}

/// Rules of Workflow
/// 1. Must be able to cancel Workflow with immediate effect
/// 2. Must check bots are free before running activity
/// 3. Must unlock bot when activity is finished
///
/// Rules of Nodes
/// Start -> Move straight on, nothing to wait for
/// Parallel -> Initial parallel moves straight on, outer parallel holds workflow until all paths
///             are finished
/// Exclusive ->
/// Trigger -> 2 flavours, fire and forget/call and response, if latter, wait for response, if former no waiting required
/// End -> Ends entire workflow as soon as any end node is hit
/// Activity -> Initial call is fire + forget then wait for response before resolving node
///
/// Select between shutdown future and current running workflow

async fn executor<T>(workflow: T) -> Result<(), Box<dyn Error>>
where
    T: Iterator,
    <T as Iterator>::Item: Node,
{
    for node in workflow {
        let status = node.run().await;
        match status {
            // return Error so it bubbles up
            Err(e) => panic!("Something gone wrong: {:#?}", e),
            Ok(_) => println!("Resolved"),
        }
    }
    Ok(())
}

use crate::workflow;
use async_trait::async_trait;
use std::fmt::Debug;
use thiserror::Error;

use super::workflow::WorkflowNodeType;

pub(crate) mod types;

#[derive(Error, Debug)]
pub enum NodeError {
    #[error("Failed - {0}")]
    Failed(String),
    #[error("Communications")]
    Communication,
}

#[async_trait]
pub trait Node: Debug + Send + Sync {
    fn kind(&self) -> WorkflowNodeType;
    fn id(&self) -> &str; // do we need id if we have the position?
    fn position(&self) -> usize; // do we need position if we have the id?
    async fn execute(&self) -> Result<Vec<workflow::Parameter>, NodeError> {
        Ok(vec![])
    }
    async fn create_msg(&self) -> workflow::Message {
        match self.execute().await {
            Ok(context) => workflow::Message {
                pointer: self.position(),
                status: workflow::NodeStatus::Success,
                context,
            },
            Err(e) => workflow::Message {
                pointer: self.position(),
                status: workflow::NodeStatus::Failed,
                context: vec![],
            },
        }
    }
    async fn run(&self) -> Result<(), NodeError>;
}

//clone_trait_object!(Node); // Don't need to clone this as we can use an Arc to clone the Box

// Rules of Workflow
// 1. Must be able to cancel Workflow with immediate effect
// 2. Must check bots are free before running activity
// 3. Must unlock bot when activity is finished
//
// Rules of Nodes
// Start -> Move straight on, nothing to wait for
// Parallel -> Initial parallel moves straight on, outer parallel holds workflow until all paths
//             are finished
// Exclusive ->
// Trigger -> 2 flavours, fire and forget/call and response, if latter, wait for response, if former no waiting required
// End -> Ends entire workflow as soon as any end node is hit
// Activity -> Initial call is fire + forget then wait for response before resolving node
//
// Select between shutdown future and current running workflow

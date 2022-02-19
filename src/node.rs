//! The interface for running a node
//!
//! All Nodes are cast to concrete types, with each type defining it's custom execution behaviour but
//! each type inherits from a common interface, the [`Node`](self::Node) trait.
//!
//! The Node trait allows us to pass around a trait object meaning we don't need knowledge of a Node's
//! specific type, only it's behaviour.

use crate::workflow;
use async_trait::async_trait;
use std::fmt::Debug;
use thiserror::Error;

use super::workflow::WorkflowNodeType;

pub(crate) mod types;

/// The error type used for defining an exception when running a node
#[derive(Error, Debug)]
pub enum NodeError {
    #[error("Failed - {0}")]
    Failed(String),
    #[error("Communications")]
    Communication,
}

/// The Node trait is to be implemented by all node types that can occur in a workflow
///
/// The Node trait is designed to be used as a trait object which allow us to erase (type erasure) the concrete
/// type and instead rely on the methods solely available through this trait.
#[async_trait]
pub trait Node: Debug + Send + Sync {
    /// Return the type of node, this is used for easily locating the Start and End nodes
    fn kind(&self) -> WorkflowNodeType;
    /// The current node ID, used by the [`Job`](crate::workflow::Job) struct to know the current node
    fn id(&self) -> &str;
    /// A pointer to the current nodes position in the [`Job.nodes`](crate::workflow::Job.nodes) collection
    fn position(&self) -> usize;
    /// The instructions for how to execute each node
    async fn execute(&self) -> Result<Vec<workflow::Parameter>, NodeError> {
        Ok(vec![])
    }
    /// Create message to be sent to the executor once the node has been executed
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
    /// The publicly exposed API for running a node
    async fn run(&self) -> Result<(), NodeError>;
}

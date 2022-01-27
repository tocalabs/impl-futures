//! The workflow module contains the structs and functions relating to the Workflow object
//! and job object.
//!
//! The Workflow struct is a 1:1 representation of the Workflow class we have defined currently
//! within the system. The Job struct is a representation of a Workflow when it is being processed
//! by the execution engine.
//!
//! The Job struct implements the Iterator trait and this contains the logic for moving from node
//! to node during execution.
use eval::Expr;
use serde::Deserialize;
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt::Debug;
use std::iter::Iterator;
use tokio::sync::mpsc::{Receiver, Sender};
use uuid::Uuid;

use crate::node::{types::*, Node};
use crate::reactor::Event;

/// A pointer indicates which other nodes a node is pointing to.
///
/// This includes the ID of the node it is pointing to and an expression (if there is one)
/// that must be evaluated for the pointer to be followed.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Pointer {
    /// ID of the node that is being pointed to
    pub points_to: String,
    /// The expression that must be evaluated to true if the pointer is to be followed
    expression: Option<String>,
}

/// A generic Parameter which is a 1:1 representation of a Parameter in the existing system.
///
/// This is used to represent objects in context.
#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Parameter {
    /// The key of the parameter
    pub key: String,
    /// The type of parameter
    #[serde(rename = "type")]
    kind: String,
    /// The value of the parameter, represented as a JSON value as we never actually use this
    pub value: Value,
}

/// A struct representing each Node within a Workflow where a node is simply an item to be executed
/// or an instruction on how to execute the Workflow.
#[derive(Deserialize, Debug, Clone)]
pub struct WorkflowNode {
    /// The type of Node
    #[serde(rename = "type")]
    kind: WorkflowNodeType,
    /// Unique ID for the node
    pub id: String,
    /// The nodes which `self` points to in the Workflow
    pointers: Vec<Pointer>,
    /// Any inputs associated with `self`
    pub parameters: Option<Vec<Parameter>>,
}

/// A 1:1 representation of the Workflow class in our existing codebase.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Workflow {
    /// Database ID to the Workflow
    id: String,
    /// ID used internally to represent the Workflow
    workflow_id: String,
    /// User who "owns" the Workflow
    associated_user_id: String,
    /// The ID of the Project that the Workflow resides in
    project_id: String,
    /// The detail of the Workflow
    workflow: Vec<WorkflowNode>,
}

/// The status representing the state any Node can find itself in
#[derive(PartialEq, Deserialize, Debug)]
pub enum WorkflowNodeStatus {
    /// The default state - indicates the node has not been run yet
    NotProcessed,
    /// The node is queued for execution but something is blocking it from running
    Pending,
    /// The node is currently being executed
    Processing,
    /// The node was executed unsuccessfully as something caused an error to occur
    Failed,
    /// The node was executed successfully
    Success,
    /// An activity is unable to be executed as the Bot is currently busy executing another process
    BotBusy,
    /// The Bot is executing the process and we are waiting for a response
    AwaitingBotResponse,
    /// The node has been cancelled via external signal
    Cancelled,
}

/// Definition for each type of node that can appear in a Workflow/Job
#[derive(PartialEq, Deserialize, Debug, Clone)]
pub enum WorkflowNodeType {
    /// The node which indicates where to start execution from
    Start,
    /// Indicates the job must run all branches in a parallel manner until the closing parallel
    /// gate
    Parallel,
    /// Indicates a conditional logic point where all paths where the expression == true must
    /// be executed, even if this results in parallel behaviour if multiple branches are true
    Exclusive,
    /// Represents an activity to be executed on a Bot
    Activity,
    /// Indicates a RESTful HTTP call is to be made
    Trigger,
    /// Signals the End of the job, the job must stop when an End node is encountered
    End,
}

/// A definition for the current running state of a Job struct.
#[derive(PartialEq, Deserialize, Debug, Clone)]
pub enum JobStatus {
    /// The default state, indicates that the job has been created but not started yet
    NotStarted,
    /// This state indicates the job is currently being run by a process
    Processing,
    /// The job has encountered an End node and has run to completion or errored
    Finished,
}

/// Job represents the workflow whilst it is running.
///
/// When a job is created from a workflow, all of the nodes are converted from a generic Node
/// struct to specific structs which represent only the data required for each node and each
/// struct has it's own implementation of what it should do when it is run.
#[derive(Debug, Clone)]
pub struct Job {
    /// The unique id of the current job, must be unique as it is used to identify the specific job.
    /// This is a uuid v4 under the hood converted to a string for ease of serialization.
    id: String,
    /// Represents the user_id of the individual who executed the job
    owner_id: String,
    /// A container type to include all the data returned from each node as the job runs
    context: Vec<Parameter>,
    /// Current is a pointer into `self.nodes` to indicate which node we are currently on within
    /// the job. It will start as None indicating the job has not started
    current: Option<usize>,
    /// Cursor map is a set of (key, value) pairs where the key is the nodeId and the value is the
    /// list of pointers coming off that node
    cursor_map: HashMap<String, Vec<Pointer>>,
    /// A list of all the nodes within the job, each node shown in a workflow will appear
    /// exactly once
    pub nodes: Vec<Box<dyn Node>>,
    /// The status represents the jobs running state
    status: JobStatus,
}

impl Job {
    /// Creates a new Job struct from a Workflow. Also requires the sender to the reactor for any
    /// activity nodes that this will create as part of the Job struct.
    /// Need to add position as property to each node
    /// Flatten pointer map to quickly scan for a nodes dependencies
    pub fn new(wf: &Workflow, tx: &Sender<Event>) -> Self {
        let mut nodes: Vec<Box<dyn Node>> = Vec::with_capacity(wf.workflow.len());
        let mut cursor_map: HashMap<String, Vec<Pointer>> = HashMap::new();
        for node in wf.workflow.iter() {
            match node.kind {
                WorkflowNodeType::Start => {
                    nodes.push(Box::new(Start::new(node)));
                }
                WorkflowNodeType::Parallel => {}
                WorkflowNodeType::Exclusive => {}
                WorkflowNodeType::Activity => {
                    nodes.push(Box::new(Activity::new(node, tx)));
                }
                WorkflowNodeType::Trigger => {}
                WorkflowNodeType::End => {
                    nodes.push(Box::new(End::new(node)));
                }
            }
            cursor_map.insert(node.id.clone(), node.pointers.clone());
        }
        Job {
            id: Uuid::new_v4().to_string(),
            owner_id: wf.associated_user_id.clone(),
            context: vec![],
            current: None,
            cursor_map,
            nodes,
            status: JobStatus::NotStarted,
        }
    }

    /// Problem with returning references is that we cannot pass the reference
    /// across a thread boundary safely so will have to introduce something like an `Arc`
    /// to satisfy the borrow checker
    fn next_node(&self, pointer: Option<usize>) -> Option<Vec<&dyn Node>> {
        if let Some(ptr) = pointer {
            let current = &**self.nodes.get(ptr)?;
            let points_to = self.cursor_map.get(current.id())?;
            let mut next_nodes: Vec<&dyn Node> = vec![];
            for path in points_to {
                next_nodes.push(&**self.nodes.iter().find(|x| path.points_to == x.id())?)
            }
            if next_nodes.is_empty() && current.kind() == WorkflowNodeType::End {
                None
            } else {
                Some(next_nodes)
            }
        } else {
            Some(vec![
                &**self
                    .nodes
                    .iter()
                    .find(|x| x.kind() == WorkflowNodeType::Start)?;
                1
            ])
        }
    }
}

pub enum NextNodes {
    Single(Box<dyn Node>),
    Multiple(Vec<Job>),
}

impl Iterator for Job {
    type Item = NextNodes;
    /// Progress to the next node in the job.
    ///
    /// The iterator will return either a single node which is next to be executed or it will
    /// return a `Vec<Job>` which indicates that there are many to execute
    fn next(&mut self) -> Option<Self::Item> {
        match self.current {
            // None indicates workflow hasn't started yet so we move onto Start node first
            None => {
                let start = self
                    .nodes
                    .iter()
                    .position(|x| x.kind() == WorkflowNodeType::Start);
                self.current = start;
                Some(NextNodes::Single(self.nodes.get(start?)?.clone()))
            }
            // Some indicates we have started the job and have processed at least 1 node.
            Some(node_index) => {
                let current_node = self.nodes.get(node_index)?;
                let node_id = current_node.id();
                let pointers = self.cursor_map.get(node_id).unwrap();
                // Nothing to iterate over if current is WorkflowNodeType::End so next will remain
                // as None
                match pointers.len().cmp(&1) {
                    Ordering::Less => {
                        // We are on the End node as there are no more pointers
                        None
                    }
                    Ordering::Equal => {
                        // Only points to one item so we can safely assume the next node is the
                        // only one being pointed to
                        self.current = self
                            .nodes
                            .iter()
                            .position(|node| node.id() == pointers[0].points_to);
                        return Some(NextNodes::Single(self.nodes.get(self.current?)?.clone()));
                    }
                    Ordering::Greater => {
                        // Points to more than one node so we must evaluate the expressions to check
                        // which nodes we should actually traverse to
                        let mut valid_pointers: Vec<&str> = vec![];
                        for pointer in pointers.iter() {
                            match &pointer.expression {
                                None => {
                                    // No expression to evaluate, follow pointer as normal
                                }
                                Some(expr) => {
                                    if Expr::new(expr)
                                        .exec()
                                        .expect("Unable to evaluate expression")
                                        .is_boolean()
                                    {
                                        valid_pointers.push(&pointer.points_to);
                                    }
                                }
                            }
                        }
                        match valid_pointers.len().cmp(&1) {
                            // If only 1 valid pointer then clearly a single move
                            Ordering::Equal => {
                                self.current = self
                                    .nodes
                                    .iter()
                                    .position(|node| node.id() == valid_pointers[0]);
                                return Some(NextNodes::Single(
                                    self.nodes.get(self.current?)?.clone(),
                                ));
                            }
                            Ordering::Less => {
                                // Return error rather than panic
                                panic!("No expression met - stopping Job");
                            }
                            Ordering::Greater => {
                                // Work out what to return based on what the current node is
                                // Parallel - return n Jobs which only go up to their closing
                                //            parallel and set `self.current` as closing parallel
                                // Exclusive - return n Jobs which contains all nodes then we do a
                                //             `select!` to finish when the first child reaches
                                //             the end
                                todo!();
                            }
                        }
                    }
                }
            }
        }
    }
}

/// async Next function

pub enum NodeStatus {
    Success,
    Failed,
}

/// Message is the struct transmitted to the executor to signal the job can make progress
pub struct Message {
    /// Index of node sending message
    pub pointer: usize,
    /// Status of the node
    pub status: NodeStatus,
    /// List of context sent back
    pub context: Vec<Parameter>,
}

///```
/// let tx, rx = mpsc
/// let job = Job::new(tx) //pass in tx for event based notification for when to progress
/// //Each node is responsible for notifying the job that it can move forward
/// //The next node function will need to take a pointer to the current node that has finished
/// // So it knows where to resume the job from
/// let next_node = job.next(None); // gets start node at very beginning
/// next_node.run().await(); // Waiting for start node to complete
/// while let Some(msg) = rx.recv().await {
///     match msg.status {
///         NodeStatus::Failed => { return Err(()) }
///         NodeStatus::Success => {
///             match job.next_nodes(msg) {
///                 Some(nodes) => {}
///             }
///             let next_nodes = job.next_nodes(msg); //next_nodes could return multiple jobs
///             if nodes.len() == 1 {
///                 node.run().await
///             }
///             for node in next_nodes {
///                 task::spawn(node.run())
///             }
///         }
///     }
///     
///     
/// }
///```
///

async fn next_node(job: &Job) -> Result<Box<dyn Node>, ()> {
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Message>(20);
    while let Some(msg) = rx.recv().await {
        let node_sender = tx.clone();
    }
    Err(())
}

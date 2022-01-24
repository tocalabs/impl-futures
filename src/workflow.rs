use eval::{to_value, Expr};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::Debug;
use std::iter::Iterator;
use tokio::sync::mpsc::Sender;
use uuid::Uuid;

use crate::node::{types::*, Node};
use crate::reactor::Event;

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Pointer {
    pub points_to: String,
    expression: Option<String>,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Parameter {
    pub key: String,
    #[serde(rename = "type")]
    kind: String,
    pub value: Value,
}

#[derive(Deserialize, Debug, Clone)]
pub struct WorkflowNode {
    #[serde(rename = "type")]
    kind: WorkflowNodeType,
    pub id: String,
    pointers: Vec<Pointer>,
    pub parameters: Option<Vec<Parameter>>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Workflow {
    id: String,
    workflow_id: String,
    associated_user_id: String,
    project_id: String,
    workflow: Vec<WorkflowNode>,
}

#[derive(PartialEq, Deserialize, Debug)]
pub enum WorkflowNodeStatus {
    NotProcessed,
    Pending,
    Processing,
    Failed,
    Success,
    BotBusy,
    AwaitingBotResponse,
    Cancelled,
    ManuallyAborted,
}

#[derive(PartialEq, Deserialize, Debug, Clone)]
pub enum WorkflowNodeType {
    Start,
    Parallel,
    Exclusive,
    Activity,
    Trigger,
    End,
}

#[derive(PartialEq, Deserialize, Debug, Clone)]
pub enum JobStatus {
    NotStarted,
    Processing,
    Finished,
}

#[derive(Debug, Clone)]
pub struct Job {
    id: String,
    owner_id: String,
    context: Vec<Parameter>,
    current: Option<Box<dyn Node>>,
    cursor_map: HashMap<String, Vec<Pointer>>,
    pub nodes: Vec<Box<dyn Node>>,
    status: JobStatus,
}

impl Job {
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
}

pub(crate) enum NextNodes {
    Single(Box<dyn Node>),
    Multiple(Vec<Job>),
}

impl Iterator for Job {
    type Item = NextNodes;

    fn next(&mut self) -> Option<Self::Item> {
        match &self.current {
            // None indicates workflow hasn't started yet so we move onto Start node first
            None => {
                let start = self
                    .nodes
                    .iter()
                    .find(|x| x.kind() == WorkflowNodeType::Start)?
                    .clone();
                self.current = Some(start.clone());
                Some(NextNodes::Single(start))
            }
            // Some indicates we have started the job and have processed at least 1 node.
            Some(t) => {
                let node_id = t.id();
                let pointers = self.cursor_map.get(node_id).unwrap();
                let mut next = None;
                // Nothing to iterate over if current is WorkflowNodeType::End so next will remain
                // as None
                let mut multi_nodes: Vec<Box<dyn Node>> = vec![];
                let borrowed = &mut multi_nodes;
                for pointer in pointers.iter() {
                    let next_id = &pointer.points_to;
                    let next_node = self
                        .nodes
                        .iter()
                        .find(|x| x.id() == next_id)
                        .unwrap()
                        .clone();

                    if t.kind() == WorkflowNodeType::Exclusive {
                        let evaluation = Expr::new(pointer.expression.as_ref()?).exec();
                        if evaluation.expect("Unable to evaluate expression") == true {
                            borrowed.push(next_node.clone());
                        }
                    } else {
                        borrowed.push(next_node.clone());
                    }

                    if borrowed.len() == 1 {
                        next = Some(NextNodes::Single(next_node.clone()))
                    } else if borrowed.len() > 1 {
                        next = Some(NextNodes::Multiple(
                            borrowed
                                .into_iter()
                                .map(|node| {
                                    let mut new_job = (*self).clone();
                                    new_job.current = Some(node.clone());
                                    new_job
                                })
                                .collect::<Vec<Job>>(),
                        )); // todo: Create jobs
                    } else {
                        panic!(format!("Unable to find the next node"));
                    }
                    // self.current = Some(next_node.clone()); // "What is going on here?"
                }
                next
            }
        }
    }
}

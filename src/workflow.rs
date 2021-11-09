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

#[derive(PartialEq, Deserialize, Debug)]
pub enum JobStatus {
    NotStarted,
    Processing,
    Finished,
}

/*
#[derive(Deserialize, Debug, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Node {
    pub id: String,
    node_id: String,
    parameters: Vec<Parameter>,
    activity_id: Option<String>,
    bot_id: Option<String>,
    #[serde(rename = "type")]
    kind: WorkflowNodeType,
    pub pointers: Vec<Pointer>,
}

impl Node {
    fn new(wf_node: WorkflowNode) -> Self {
        let mut activity_id: Option<String> = None;
        let params = match wf_node.parameters {
            Some(params) => {
                activity_id = params.iter().find(|x| x.key == "ActivityId").map(|param| {
                    serde_json::from_value::<String>(param.value.clone())
                        .expect("Unable to deserialize Activity Id")
                });
                params
            }
            None => vec![],
        };
        Node {
            id: Uuid::new_v4().to_string(),
            node_id: wf_node.id,
            parameters: params,
            activity_id,
            bot_id: None,
            kind: wf_node.kind,
            pointers: wf_node.pointers,
        }
    }
    pub fn next(&self, nodes: &[Node]) -> Vec<Node> {
        let next_ids: Vec<String> = self.pointers.iter().cloned().map(|x| x.points_to).collect();
        nodes
            .iter()
            .cloned()
            .filter(|nd| next_ids.contains(&nd.node_id))
            .collect()
    }

    pub fn get_next(&self, job: &mut Job) {
        job.current
            .remove(job.current.iter().position(|node| *node == *self).unwrap());
        let next = self.next(&job.nodes);
        let mut current_pointers: Vec<String> = vec![];
        for red in job.current.iter() {
            current_pointers.append(
                &mut red
                    .pointers
                    .iter()
                    .cloned()
                    .map(|x| x.points_to)
                    .collect::<Vec<String>>(),
            );
        }
        if !next
            .iter()
            .any(|nxt| current_pointers.contains(&nxt.node_id))
        {
            job.current.extend(next);
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct Job {
    id: String,
    owner_id: String,
    context: Vec<Parameter>,
    pub current: Vec<Node>,
    pub nodes: Vec<Node>,
    status: JobStatus,
}

#[derive(Deserialize, Debug)]
pub struct ExecutionRequest {
    pub workflow_id: String,
    pub job_args: Option<Vec<Parameter>>,
}

impl Job {
    pub fn new(workflow: Workflow, request: ExecutionRequest) -> Self {
        let nodes: Vec<Node> = workflow
            .workflow
            .iter()
            .map(|nd| Node::new(nd.clone()))
            .collect();

        let start_node = nodes
            .iter()
            .find(|nd| nd.kind == WorkflowNodeType::Start)
            .expect("No Start node present!")
            .clone();

        Job {
            id: Uuid::new_v4().to_string(),
            owner_id: workflow.associated_user_id,
            context: match request.job_args {
                Some(args) => args,
                None => vec![],
            },
            current: vec![start_node],
            nodes,
            status: JobStatus::NotStarted,
        }
    }
}
*/

#[derive(Debug)]
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

impl Iterator for Job {
    type Item = Box<dyn Node>;

    fn next(&mut self) -> Option<Self::Item> {
        match &self.current {
            // None indicates workflow hasn't started yet
            None => {
                let start = self
                    .nodes
                    .iter()
                    .find(|x| x.kind() == WorkflowNodeType::Start)?
                    .clone();
                self.current = Some(start.clone());
                Some(start)
            }
            Some(t) => {
                let node_id = t.id();
                let pointers = self.cursor_map.get(node_id).unwrap();
                let mut next = None;
                for pointer in pointers.iter() {
                    let next_id = &pointer.points_to;
                    let next_node = self
                        .nodes
                        .iter()
                        .find(|x| x.id() == next_id)
                        .unwrap()
                        .clone();
                    self.current = Some(next_node.clone());
                    next = Some(next_node);
                }
                next
            }
        }
    }
}

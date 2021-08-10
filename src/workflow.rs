use serde::Deserialize;
use serde_json::Value;
use std::fmt::Debug;
use std::iter::Iterator;
use uuid::Uuid;

use super::node::Node;

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Pointer {
    pub points_to: String,
    expression: Option<String>,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Parameter {
    key: String,
    #[serde(rename = "type")]
    kind: String,
    value: Value,
}

#[derive(Deserialize, Debug, Clone)]
pub struct WorkflowNode {
    #[serde(rename = "type")]
    kind: WorkflowNodeType,
    id: String,
    pointers: Vec<Pointer>,
    parameters: Option<Vec<Parameter>>,
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
    JobArgs,
    Parallel,
    Exclusive,
    Activity,
    Error,
    Output,
    Trigger,
    End,
    Label,
    StatelessAction,
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

enum IterResult {
    Many(Vec<Job>),
    Single(Box<dyn Node>),
}
struct Job {
    id: String,
    owner_id: String,
    context: Vec<Parameter>,
    pub current: Vec<Box<dyn Node>>,
    pub nodes: Vec<Box<dyn Node>>,
    status: JobStatus,
}

impl Iterator for Workflow {
    type Item = IterResult;

    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}

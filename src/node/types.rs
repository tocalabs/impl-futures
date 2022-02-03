use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll};

use async_trait::async_trait;
use tokio::sync::mpsc::Sender;

use crate::reactor::Event;
use crate::workflow;
use crate::workflow::{Parameter, WorkflowNode, WorkflowNodeType};

use super::{Node, NodeError};

#[derive(Clone, Debug)]
pub struct Start {
    id: String,
    job_channel: Sender<workflow::Message>,
    position: usize,
}

#[derive(Clone, Debug)]
pub struct End {
    id: String,
    job_channel: Sender<workflow::Message>,
    position: usize,
}

#[derive(Clone, Debug)]
pub struct Activity {
    id: String,
    activity_id: String,
    description: String,
    fail_on_error: bool,
    waker_tx: Sender<Event>,
    job_channel: Sender<workflow::Message>,
    position: usize,
}

#[derive(Debug)]
pub enum Parallel {
    Opening {
        id: String,
        job_channel: Sender<workflow::Message>,
        position: usize,
    },
    Closing {
        id: String,
        job_channel: Sender<workflow::Message>,
        position: usize,
        /// AtomicUsize does not implement Clone so we cannot trivially clone Parallel without
        /// using an `Rc` or `Arc`
        dependencies: AtomicUsize,
        dependencies_met: AtomicUsize,
    },
}

impl Parallel {
    pub(crate) fn new(
        wf: &WorkflowNode,
        job_channel: Sender<workflow::Message>,
        position: usize,
        dependencies: Option<usize>,
    ) -> Self {
        match dependencies {
            None => Parallel::Opening {
                id: wf.id.to_string(),
                job_channel,
                position,
            },
            Some(dependencies) => Parallel::Closing {
                id: wf.id.to_string(),
                job_channel,
                position,
                dependencies: AtomicUsize::new(dependencies),
                dependencies_met: AtomicUsize::new(0),
            },
        }
    }
}

#[derive(Clone, Debug)]
pub struct Exclusive {
    id: String,
    job_channel: Sender<workflow::Message>,
    position: usize,
}

enum FutureStatus {
    Start,
    RequestSent,
}

struct ActivityFuture {
    activity: Activity,
    state: FutureStatus,
    outputs: Vec<Parameter>,
}

impl ActivityFuture {
    fn new(activity: &Activity) -> Self {
        ActivityFuture {
            activity: activity.clone(),
            state: FutureStatus::Start,
            outputs: vec![],
        }
    }
}

impl Future for ActivityFuture {
    type Output = Vec<Parameter>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let me = self.get_mut();
        match me.state {
            FutureStatus::Start => {
                let waker = cx.waker().clone();
                let tx = me.activity.waker_tx.clone();
                let event = Event::Activity {
                    node_id: uuid::Uuid::new_v4().to_string(),
                    activity_id: me.activity.activity_id.clone(),
                    waker,
                };
                tx.try_send(event)
                    .expect("Unable to send error! Failing workflow");

                me.state = FutureStatus::RequestSent;
                Poll::Pending
            }
            FutureStatus::RequestSent => Poll::Ready(me.outputs.clone()),
        }
    }
}

impl Start {
    pub fn new(wf: &WorkflowNode, job_channel: Sender<workflow::Message>, position: usize) -> Self {
        Start {
            id: wf.id.clone(),
            job_channel,
            position,
        }
    }
}

#[async_trait]
impl Node for Start {
    fn kind(&self) -> WorkflowNodeType {
        WorkflowNodeType::Start
    }
    fn id(&self) -> &str {
        &self.id
    }

    fn position(&self) -> usize {
        self.position
    }

    async fn run(&self) -> Result<(), NodeError> {
        self.job_channel.send(self.create_msg().await).await;
        Ok(())
    }
}

impl End {
    pub fn new(wf: &WorkflowNode, job_channel: Sender<workflow::Message>, position: usize) -> Self {
        End {
            id: wf.id.clone(),
            job_channel,
            position,
        }
    }
}

#[async_trait]
impl Node for End {
    fn kind(&self) -> WorkflowNodeType {
        WorkflowNodeType::End
    }
    fn id(&self) -> &str {
        &self.id
    }
    fn position(&self) -> usize {
        self.position
    }
    async fn run(&self) -> Result<(), NodeError> {
        self.job_channel.send(self.create_msg().await).await;
        Ok(())
    }
}

#[async_trait]
impl Node for Parallel {
    fn kind(&self) -> WorkflowNodeType {
        WorkflowNodeType::Start
    }
    fn id(&self) -> &str {
        match &self {
            Parallel::Opening { id, .. } => id,
            Parallel::Closing { id, .. } => id,
        }
    }

    fn position(&self) -> usize {
        match &self {
            Parallel::Opening { position, .. } => *position,
            Parallel::Closing { position, .. } => *position,
        }
    }

    async fn run(&self) -> Result<(), NodeError> {
        match &self {
            Parallel::Opening { job_channel, .. } => {
                job_channel.send(self.create_msg().await).await;
            }
            Parallel::Closing {
                dependencies,
                dependencies_met,
                job_channel,
                ..
            } => {
                let _ = dependencies_met.fetch_add(1, Ordering::Acquire);
                let new = dependencies_met.load(Ordering::Relaxed);
                if new == dependencies.load(Ordering::Acquire) {
                    job_channel.send(self.create_msg().await).await;
                }
            }
        }
        Ok(())
    }
}

impl Activity {
    pub fn new(
        wf: &WorkflowNode,
        waker_channel: &Sender<Event>,
        job_channel: Sender<workflow::Message>,
        position: usize,
    ) -> Self {
        let mut activity_id: Option<String> = None;
        let mut description: Option<String> = None;
        let mut fail_on_error: Option<bool> = None;
        for param in wf.parameters.as_ref().unwrap().iter() {
            match param.key.to_lowercase().as_ref() {
                "activityid" => activity_id = Some(param.value.as_str().unwrap().to_string()),
                "description" => description = Some(param.value.as_str().unwrap().to_string()),
                "failonerror" => fail_on_error = Some(param.value.as_bool().unwrap()),
                _ => continue,
            }
        }
        Activity {
            id: wf.id.clone(),
            activity_id: activity_id.unwrap(),
            description: description.unwrap(),
            fail_on_error: fail_on_error.unwrap(),
            waker_tx: waker_channel.clone(),
            job_channel,
            position,
        }
    }
}

#[async_trait]
impl Node for Activity {
    fn kind(&self) -> WorkflowNodeType {
        WorkflowNodeType::Activity
    }
    fn id(&self) -> &str {
        &self.id
    }

    fn position(&self) -> usize {
        self.position
    }

    async fn execute(&self) -> Result<Vec<workflow::Parameter>, NodeError> {
        let future = ActivityFuture::new(self);
        let outputs = future.await;
        Ok(outputs)
    }

    async fn run(&self) -> Result<(), NodeError> {
        self.job_channel.send(self.create_msg().await).await;
        Ok(())
    }
}

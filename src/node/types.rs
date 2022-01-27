use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::AtomicUsize;
use std::task::{Context, Poll};

use async_trait::async_trait;
use tokio::sync::mpsc::Sender;

use crate::reactor::Event;
use crate::workflow;
use crate::workflow::{WorkflowNode, WorkflowNodeType};

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
pub struct Parallel {
    id: String,
    dependencies: AtomicUsize, // AtomicUsize does not implement Clone so we may have to think carefully about references here
    dependencies_met: AtomicUsize,
    exec_tx: Sender<workflow::Message>,
    position: usize,
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
}

impl ActivityFuture {
    fn new(activity: &Activity) -> Self {
        ActivityFuture {
            activity: activity.clone(),
            state: FutureStatus::Start,
        }
    }
}

impl Future for ActivityFuture {
    type Output = ();

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
            FutureStatus::RequestSent => Poll::Ready(()),
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
    async fn execute(&self) -> Result<(), NodeError> {
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
    async fn execute(&self) -> Result<Vec<workflow::Parameter>, NodeError> {
        Ok(vec![])
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
        future.await;
        Ok(())
    }

    async fn run(&self) -> Result<(), NodeError> {
        self.job_channel.send(workflow::Message { No }).await;
        Ok(())
    }
}

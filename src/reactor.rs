use std::iter::Iterator;
use std::sync::mpsc::Receiver;
use std::task::Waker;

struct Reactor {
    rx: Receiver<Waker>,
}

struct WorkflowNode {
    id: i16,
    pointers: Pointer,
}

struct Pointer {
    pointers: Vec<String>,
}

struct Workflow {
    nodes: Vec<WorkflowNode>,
}

impl Iterator for Workflow {
    type Item = WorkflowNode;

    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}

trait Transport {
    fn send();
    fn receive();
}

struct Comms;

impl Reactor {
    fn new(rx: Receiver<Waker>) -> Self {
        Reactor { rx }
    }

    async fn get_wakers(&self) {
        while let Ok(message) = self.rx.recv() {
            message.wake();
        }
    }
}

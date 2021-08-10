use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::task::{Context, Poll, Waker};
use std::thread;
use std::time::{Duration, Instant};
use tokio::task;
mod node;
mod reactor;
mod workflow;
use tokio::fs;

use tokio::time::sleep;

async fn test() -> () {
    sleep(Duration::from_secs(5)).await;
    ()
}

#[derive(Clone)]
struct Activity(usize);

impl Node for Activity {
    type Output = ActivityFuture;
    fn run(&self, sender: Option<&Sender<Waker>>) -> Self::Output {
        let tx = sender.expect("Has to be sender sent to Activity");
        ActivityFuture::new(self.clone(), tx)
    }
}

enum FutureStatus {
    Start,
    RequestSent,
}

struct ActivityFuture {
    activity: Activity,
    waker_channel: Sender<Waker>,
    state: FutureStatus,
}

impl ActivityFuture {
    fn new(activity: Activity, sender: &Sender<Waker>) -> Self {
        ActivityFuture {
            activity,
            waker_channel: sender.clone(),
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
                let tx = me.waker_channel.clone();
                tx.send(waker).unwrap();
                me.state = FutureStatus::RequestSent;
                Poll::Pending
            }
            FutureStatus::RequestSent => Poll::Ready(()),
        }
    }
}

async fn reactor(mut rx: Receiver<Waker>) {
    let mut count = 0;
    let start = Instant::now();
    while let Ok(message) = rx.recv() {
        message.wake();
        count += 1;
        if count % 1_000_000 == 0 {
            println!(
                "-------------------- Duration == {:#?} ------------------------",
                (Instant::now() - start) / 1_000_000
            );
            //break;
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), io::Error> {
    // test().await;
    let wf_json = fs::read_to_string("workflow.json").await?;
    let wf: workflow::Workflow = serde_json::from_str(&wf_json)?;
    println!("{:#?}", wf);
    // let (tx, rx) = channel();
    // //let th = thread::spawn(move || reactor(rx));
    // let th = task::spawn(reactor(rx));
    // for i in 0..=1_000_000 {
    //     let activity = Activity(i);
    //     task::spawn(activity.run(Some(&tx)));
    // }
    // th.await?;

    //th.join().unwrap();

    Ok(())
}

//! A small module for managing and tracking asynchronous task join handles.
//!
//! Provides a thread-safe way to collect and wait for multiple async tasks
//! to complete. It uses a combination of channels and a futures unordered collection
//! to handle both existing and new tasks that may be added while waiting.
//!
//! The alternatives are:
//! With FuturesUnordered alone, or tokio::JoinSet, you have to lock it to wait on the result, but if it's
//! locked, you can't add new handles. tokio_util's TaskTracker does allow insertion without mutable access,
//! however it doesn't actually allow you to handle the result of the tasks.
//!

use futures::stream::FuturesUnordered;
use log::debug;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

#[derive(Debug)]
pub struct JoinHandles {
    handles: Arc<Mutex<FuturesUnordered<JoinHandle<()>>>>,
    rx: Arc<Mutex<mpsc::UnboundedReceiver<JoinHandle<()>>>>,
    tx: mpsc::UnboundedSender<JoinHandle<()>>,
}
impl JoinHandles {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel::<JoinHandle<()>>();
        Self {
            handles: Arc::new(Mutex::new(FuturesUnordered::new())),
            rx: Arc::new(Mutex::new(rx)),
            tx,
        }
    }
    pub fn push(&self, handle: JoinHandle<()>) {
        self.tx.send(handle).unwrap();
    }
    pub async fn start(&self) {
        use futures::StreamExt as _;
        let mut handles = self.handles.lock().unwrap();
        let mut rx = self.rx.lock().unwrap();

        loop {
            tokio::select! {
                hdl_ret = handles.next() => {
                    let result = hdl_ret.unwrap();
                    result.unwrap();
                    debug!("JoinHandles::wait_all: handle completed");
                }
                handle = rx.recv() => {
                    debug!("JoinHandles::wait_all: adding new handle");
                    handles.push(handle.unwrap());
                }
            }
        }
    }
}

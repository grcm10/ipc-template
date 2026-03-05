use crate::ipc_manager::{EchoHandler, IpcManager};
use std::error::Error;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
pub struct Listener {
    listener: TcpListener,
}

impl Listener {
    pub async fn bind(addr: &str) -> Result<Self, std::io::Error> {
        let listener = TcpListener::bind(addr).await?;
        Ok(Self { listener })
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn Error>> {
        loop {
            // Asynchronously wait for an inbound socket.
            let (socket, _) = self.listener.accept().await?;
            let handler = Arc::new(EchoHandler);
            tokio::spawn(async move {
                let (_, worker) = IpcManager::new::<TcpStream>(socket, handler);

                let worker_handle = tokio::spawn(worker);
                let _ = worker_handle.await;
            });
        }
    }
}

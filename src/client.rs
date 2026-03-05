use crate::ipc_manager::{EchoHandler, IpcManager, RequestSender};
use rand::RngExt;
use std::error::Error;
use std::sync::Arc;
use tokio::net::TcpStream;

pub struct Client {
    pub connection: Connection,
}

impl Client {
    pub async fn connect(stream: TcpStream) -> Result<Self, Box<dyn Error>> {
        let handler = Arc::new(EchoHandler);
        let (manager, _) = IpcManager::new::<TcpStream>(stream, handler);
        let shared_manager = Arc::new(manager);
        let connection = Connection::new(shared_manager);
        Ok(Self { connection })
    }
}

pub struct Connection {
    sender: Arc<dyn RequestSender>,
}

impl Connection {
    pub fn new(sender: Arc<dyn RequestSender>) -> Self {
        Self { sender }
    }

    pub async fn send(&self) -> Result<(), Box<dyn Error>> {
        let mut rng = rand::rng();
        for _ in 0..10 {
            let len = rng.random_range(1..5000);
            let random_data: Vec<u8> = (0..len).map(|_| rng.random::<u8>()).collect();
            println!("len: {:?}", len);
            // Send data: The codec automatically prefixes the length
            self.send_with_retry(random_data, 3).await?;
        }
        Ok(())
    }

    pub async fn send_with_retry(
        &self,
        data: Vec<u8>,
        max_retries: usize,
    ) -> Result<Vec<u8>, String> {
        let mut attempts = 0;
        while attempts <= max_retries {
            match tokio::time::timeout(
                std::time::Duration::from_secs(2),
                self.sender.send_request(data.clone()),
            )
            .await
            {
                Ok(Ok(response)) => return Ok(response),
                _ => {
                    attempts += 1;
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                }
            }
        }
        Err("Max retries exceeded".into())
    }
}

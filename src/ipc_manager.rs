use async_trait::async_trait;
use bytes::Bytes;
use bytes::BytesMut;
use futures::{SinkExt, StreamExt, stream::SplitSink, stream::SplitStream};
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

const FREQUENCY: usize = 100;
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[async_trait]
pub trait RequestSender: Send + Sync {
    async fn send_request(&self, data: Vec<u8>) -> Result<Vec<u8>, String>;
}

#[async_trait]
impl RequestSender for IpcManager {
    async fn send_request(&self, data: Vec<u8>) -> Result<Vec<u8>, String> {
        let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
        let req = self.create_request(data, resp_tx);

        self.tx
            .send(req)
            .await
            .map_err(|_| "Channel closed".to_string())?;

        resp_rx.await.map_err(|_| "Canceled".to_string())?
    }
}

#[async_trait::async_trait]
pub trait PacketHandler: Send + Sync {
    async fn handle(&self, request: Vec<u8>) -> Vec<u8>;
}

pub struct EchoHandler;

#[async_trait::async_trait]
impl PacketHandler for EchoHandler {
    async fn handle(&self, request: Vec<u8>) -> Vec<u8> {
        request
    }
}

pub struct PendingRequest {
    data: Vec<u8>,
    response_tx: oneshot::Sender<Result<Vec<u8>, String>>,
}

struct IpcWorker<T> {
    sink: SplitSink<Framed<T, LengthDelimitedCodec>, Bytes>,
    stream: SplitStream<Framed<T, LengthDelimitedCodec>>,
    rx: mpsc::Receiver<PendingRequest>,
    map: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Vec<u8>, String>>>>>,
    handler: Arc<dyn PacketHandler>,
}

impl<T> IpcWorker<T>
where
    T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    async fn run(mut self) {
        loop {
            tokio::select! {
                // Handle the request from mpsc.
                Some(p) = self.rx.recv() => {
                    if let Err(e) = self.handle_send(p).await {
                        eprintln!("Send error: {}", e);
                        break;
                    }
                }
                // Handle the response from TCP.
                Some(res) = self.stream.next() => {
                    match res {
                        Ok(bytes) => self.handle_recv(bytes).await,
                        Err(e) => {
                            eprintln!("Network recv error: {}", e);
                            break;
                        }
                    }
                }
                else => break, //Shutdown when any of connection is lost.
            }
        }
    }

    async fn handle_send(&mut self, p: PendingRequest) -> Result<(), Box<dyn Error>> {
        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
        {
            let mut guard = self.map.lock().await;
            guard.insert(id, p.response_tx);
        }
        let mut v = id.to_be_bytes().to_vec();
        v.extend(p.data);
        self.sink.send(Bytes::from(v)).await?;
        Ok(())
    }

    async fn handle_recv(&mut self, bytes: BytesMut) {
        if bytes.len() < 8 {
            return;
        }
        let id_bytes: [u8; 8] = bytes[0..8].try_into().unwrap();
        let id = u64::from_be_bytes(id_bytes);
        let response_body = self.handler.handle(bytes[8..].to_vec()).await;

        let mut guard = self.map.lock().await;
        if let Some(response_tx) = guard.remove(&id) {
            let _ = response_tx.send(Ok(response_body)); // use let_ to ignore the return value when the send is failed due to timeout.
        }
    }
}

pub struct IpcManager {
    pub tx: mpsc::Sender<PendingRequest>,
}

impl IpcManager {
    pub fn new<T>(
        stream: T,
        handler: Arc<dyn PacketHandler>,
    ) -> (Self, impl Future<Output = ()> + Send + 'static)
    where
        T: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let (tx, rx) = mpsc::channel::<PendingRequest>(FREQUENCY);

        let framed = Framed::new(stream, LengthDelimitedCodec::new());
        let (sink, stream) = framed.split();
        let map = Arc::new(Mutex::new(HashMap::new()));
        let manager = Self { tx };

        let worker = IpcWorker {
            sink,
            stream,
            rx,
            map,
            handler,
        };
        (manager, worker.run())
    }

    pub fn create_request(
        &self,
        data: Vec<u8>,
        res: oneshot::Sender<Result<Vec<u8>, String>>,
    ) -> PendingRequest {
        PendingRequest {
            data,
            response_tx: res,
        }
    }
}

use ipc_template::server::Listener;
use std::env;
use std::error::Error;
const DEFAULT_ADDR: &str = "127.0.0.1:8080";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let addr = env::args()
        .nth(1)
        .unwrap_or_else(|| DEFAULT_ADDR.to_string());
    let mut l = Listener::bind(addr.as_str()).await?;
    l.run().await
}

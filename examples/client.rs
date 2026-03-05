use ipc_template::client::Client;
use std::error::Error;
use std::net::SocketAddr;
use structopt::StructOpt;
use tokio::net::TcpStream;

#[derive(StructOpt)]
struct Opt {
    #[structopt(required = true, min_values = 1)]
    name: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let opt = Opt::from_args();
    let addr = opt
        .name
        .first()
        .ok_or("this program requires at least one argument")?;
    let addr = addr.parse::<SocketAddr>()?;
    let stream = TcpStream::connect(addr).await?;
    let client = Client::connect(stream).await?;
    client.connection.send().await?;
    Ok(())
}

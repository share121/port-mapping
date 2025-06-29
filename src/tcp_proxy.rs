use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};

#[derive(Debug)]
pub struct TcpProxy {
    pub listen: String,
    pub upstream: String,
}

impl TcpProxy {
    pub fn new(listen: String, upstream: String) -> Self {
        Self { listen, upstream }
    }

    pub async fn run(self: Arc<Self>) -> Result<(), std::io::Error> {
        let listener = TcpListener::bind(&self.listen).await?;
        println!(
            "TCP proxy listening on {} -> {}",
            self.listen, self.upstream
        );
        loop {
            let (mut downstream, _) = listener.accept().await?;
            let self_clone = self.clone();
            tokio::spawn(async move {
                let mut upstream = match TcpStream::connect(&self_clone.upstream).await {
                    Ok(stream) => stream,
                    Err(e) => {
                        println!("Failed to connect to upstream: {}", e);
                        return;
                    }
                };
                match tokio::io::copy_bidirectional(&mut downstream, &mut upstream).await {
                    Ok(_) => println!("TCP proxy connection closed"),
                    Err(e) => println!("TCP proxy connection error: {}", e),
                };
            });
        }
    }
}

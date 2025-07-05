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
            "[info][tcp] Listening on {} -> {}",
            self.listen, self.upstream
        );
        loop {
            let (mut downstream, _) = listener.accept().await?;
            let self_clone = self.clone();
            tokio::spawn(async move {
                let mut upstream = match TcpStream::connect(&self_clone.upstream).await {
                    Ok(stream) => stream,
                    Err(e) => {
                        eprintln!(
                            "[warning][tcp] {} failed to connect to {}: {e}",
                            self_clone.listen, self_clone.upstream
                        );
                        return;
                    }
                };
                match tokio::io::copy_bidirectional(&mut downstream, &mut upstream).await {
                    Ok((a, b)) => println!(
                        "[info][tcp] Connection closed: {} Send {a}B to {} and receive {b}B",
                        self_clone.listen, self_clone.upstream
                    ),
                    Err(e) => eprintln!(
                        "[warning][tcp] Connection error {}->{}: {e}",
                        self_clone.listen, self_clone.upstream
                    ),
                };
            });
        }
    }
}

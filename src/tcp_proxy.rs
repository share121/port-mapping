use std::{fmt::Display, sync::Arc};
use tokio::net::{TcpListener, TcpStream};

#[derive(Debug)]
pub struct TcpProxy {
    pub listen: String,
    pub upstream: String,
}

impl Display for TcpProxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}->{}", self.listen, self.upstream)
    }
}

impl TcpProxy {
    pub fn new(listen: String, upstream: String) -> Self {
        Self { listen, upstream }
    }

    pub async fn run(self: Arc<Self>) -> std::io::Result<()> {
        let listener = TcpListener::bind(&self.listen).await?;
        println!("[info][tcp][{self}] Listening");
        loop {
            let mut downstream = match listener.accept().await {
                Ok((downstream, _)) => downstream,
                Err(e) => {
                    eprintln!("[warning][tcp][{self}] Failed to accept connection: {e}");
                    continue;
                }
            };
            let self_clone = self.clone();
            tokio::spawn(async move {
                let mut upstream = match TcpStream::connect(&self_clone.upstream).await {
                    Ok(stream) => stream,
                    Err(e) => {
                        eprintln!("[warning][tcp][{self_clone}] Failed to connect: {e}");
                        return;
                    }
                };
                match tokio::io::copy_bidirectional(&mut downstream, &mut upstream).await {
                    Ok((a, b)) => println!(
                        "[info][tcp][{self_clone}] Connection closed: {} Send {a}B to {} and receive {b}B",
                        self_clone.listen, self_clone.upstream
                    ),
                    Err(e) => eprintln!("[warning][tcp][{self_clone}] Connection error: {e}"),
                };
            });
        }
    }
}

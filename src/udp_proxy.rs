use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::{
    net::UdpSocket,
    select,
    sync::mpsc::{self, Sender},
};

#[derive(Debug)]
pub struct UdpProxy {
    pub listen: String,
    pub upstream: String,
    pub buffer_size: usize,
}

impl UdpProxy {
    pub fn new(listen: String, upstream: String, buffer_size: usize) -> Self {
        Self {
            listen,
            upstream,
            buffer_size,
        }
    }

    pub async fn run(self: Arc<Self>) -> Result<(), std::io::Error> {
        let server = Arc::new(UdpSocket::bind(&self.listen).await?);
        println!(
            "[info][udp] Listening on {} -> {}",
            self.listen, self.upstream
        );
        let mut buf = Vec::with_capacity(self.buffer_size);
        let mut map: HashMap<SocketAddr, Sender<Vec<u8>>> = HashMap::new();
        loop {
            unsafe {
                buf.set_len(self.buffer_size);
            }
            let (len, addr) = server.recv_from(&mut buf).await?;
            unsafe {
                buf.set_len(len);
            }
            match map.get_mut(&addr) {
                Some(tx) => {
                    tx.send(buf.clone()).await.unwrap();
                }
                None => {
                    let (tx, mut rx) = mpsc::channel(1);
                    let self_clone = self.clone();
                    let server_clone = server.clone();
                    map.insert(addr, tx);
                    tokio::spawn(async move {
                        let client = match UdpSocket::bind("localhost:0").await {
                            Ok(client) => client,
                            Err(e) => {
                                eprintln!(
                                    "[warning][udp] {} failed to connect to {}: {e}",
                                    self_clone.listen, self_clone.upstream
                                );
                                return;
                            }
                        };
                        client.connect(&self_clone.upstream).await.unwrap();
                        let mut buf = Vec::with_capacity(self_clone.buffer_size);
                        loop {
                            unsafe {
                                buf.set_len(self_clone.buffer_size);
                            }
                            select! {
                                Some(received) = rx.recv() => {
                                    client.send(&received).await.unwrap();
                                }
                                Ok(len) = client.recv(&mut buf) => {
                                    unsafe {
                                        buf.set_len(len);
                                    }
                                    server_clone.send_to(&buf[..len], &addr).await.unwrap();
                                }
                            }
                        }
                    });
                }
            }
        }
    }
}

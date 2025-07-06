use dashmap::DashMap;
use std::{fmt::Display, sync::Arc, time::Duration};
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

impl Display for UdpProxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}->{}", self.listen, self.upstream)
    }
}

impl UdpProxy {
    pub fn new(listen: String, upstream: String, buffer_size: usize) -> Self {
        Self {
            listen,
            upstream,
            buffer_size,
        }
    }

    pub async fn run(self: Arc<Self>) -> std::io::Result<()> {
        let server = Arc::new(UdpSocket::bind(&self.listen).await?);
        println!("[info][udp][{self}] Listening");
        let map: Arc<DashMap<_, Sender<Vec<u8>>>> = Arc::new(DashMap::new());
        let mut buf = Vec::with_capacity(self.buffer_size);
        unsafe {
            buf.set_len(self.buffer_size);
        }
        loop {
            let (len, addr) = match server.recv_from(&mut buf).await {
                Ok(res) => res,
                Err(e) => {
                    eprintln!("[warning][udp][{self}] Failed to recv from downstream: {e}");
                    continue;
                }
            };
            match map.get(&addr) {
                Some(tx) => {
                    if let Err(e) = tx.send(buf[..len].to_vec()).await {
                        eprintln!("[warning][udp][{self}] Tokio channel error: {e}");
                        continue;
                    }
                }
                None => {
                    let (tx, mut rx) = mpsc::channel(100);
                    let self_clone = self.clone();
                    let server_clone = server.clone();
                    if let Err(e) = tx.send(buf[..len].to_vec()).await {
                        eprintln!("[warning][udp][{self}] Tokio channel error: {e}");
                    }
                    map.insert(addr, tx);
                    let map_clone = map.clone();
                    tokio::spawn(async move {
                        let client = match UdpSocket::bind("127.0.0.1:0").await {
                            Ok(client) => client,
                            Err(e) => {
                                eprintln!(
                                    "[warning][udp][{self_clone}] Failed to bind client socket: {e}"
                                );
                                return;
                            }
                        };
                        if let Err(e) = client.connect(&self_clone.upstream).await {
                            eprintln!(
                                "[warning][udp][{self_clone}] Failed to connect to upstream: {e}"
                            );
                            return;
                        };
                        let mut buf = Vec::with_capacity(self_clone.buffer_size);
                        unsafe {
                            buf.set_len(self_clone.buffer_size);
                        }
                        loop {
                            select! {
                                Some(received) = rx.recv() => {
                                    if let Err(e) = client.send(&received).await {
                                        eprintln!(
                                            "[warning][udp][{self_clone}] Failed to send to upstream: {e}"
                                        );
                                    }
                                }
                                Ok(len) = client.recv(&mut buf) => {
                                    if let Err(e) = server_clone.send_to(&buf[..len], &addr).await {
                                        eprintln!(
                                            "[warning][udp][{self_clone}] Failed to send to downstream: {e}"
                                        );
                                    }
                                }
                                _ = tokio::time::sleep(Duration::from_secs(60)) => {
                                    println!(
                                        "[info][udp][{self_clone}] No data transport for 60 seconds, closing connection"
                                    );
                                    break;
                                }
                            }
                        }
                        map_clone.remove(&addr);
                    });
                }
            }
        }
    }
}

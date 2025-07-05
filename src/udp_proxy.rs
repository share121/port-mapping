use std::{collections::HashMap, fmt::Display, sync::Arc, time::Duration};
use tokio::{
    net::UdpSocket,
    select,
    sync::{
        RwLock,
        mpsc::{self, Sender},
    },
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
        let map: Arc<RwLock<HashMap<_, Sender<Vec<u8>>>>> = Arc::new(RwLock::new(HashMap::new()));
        let mut buf = Vec::with_capacity(self.buffer_size);
        unsafe {
            buf.set_len(self.buffer_size);
        }
        loop {
            let (len, addr) = server.recv_from(&mut buf).await?;
            match map.read().await.get(&addr) {
                Some(tx) => {
                    tx.send(buf[..len].to_vec()).await.unwrap();
                }
                None => {
                    let (tx, mut rx) = mpsc::channel(1);
                    let self_clone = self.clone();
                    let server_clone = server.clone();
                    map.write().await.insert(addr, tx);
                    let map_clone = map.clone();
                    tokio::spawn(async move {
                        let client = match UdpSocket::bind("localhost:0").await {
                            Ok(client) => Arc::new(client),
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
                                    let client_clone = client.clone();
                                    tokio::spawn(async move {
                                        client_clone.send(&received).await.unwrap();
                                    });
                                }
                                Ok(len) = client.recv(&mut buf) => {
                                    let server_clone = server_clone.clone();
                                    let data = buf[..len].to_vec();
                                    tokio::spawn(async move {
                                        server_clone.send_to(&data, &addr).await.unwrap();
                                    });
                                }
                                _ = tokio::time::sleep(Duration::from_secs(60)) => {
                                    break;
                                }
                            }
                        }
                        map_clone.write().await.remove(&addr);
                    });
                }
            }
        }
    }
}

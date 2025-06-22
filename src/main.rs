use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    net::SocketAddr,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::{
    fs::File,
    io::{AsyncBufReadExt, BufReader},
    net::{TcpListener, TcpStream, UdpSocket},
    sync::Mutex,
};

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
enum Protocol {
    Tcp,
    Udp,
}

#[derive(Debug)]
struct MappingRuleEntry {
    listen: String,
    upstream: String,
    protocol: Protocol,
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
enum MappingRuleParseError {
    Empty,
    InvalidFormat(String),
    InvalidProtocol(String, String),
}

impl MappingRuleEntry {
    fn parse(line: &str) -> Result<Self, MappingRuleParseError> {
        let parts = line.split('#').next().ok_or(MappingRuleParseError::Empty)?;
        let parts: Vec<&str> = parts.split_whitespace().collect();
        if parts.len() != 3 {
            return Err(MappingRuleParseError::InvalidFormat(line.to_string()));
        }
        let listen = if parts[0].starts_with(':') {
            format!("0.0.0.0{}", parts[0])
        } else {
            parts[0].to_string()
        };
        let upstream = if parts[1].starts_with(':') {
            format!("localhost{}", parts[1])
        } else {
            parts[1].to_string()
        };
        let protocol = match parts[2].to_lowercase().as_str() {
            "udp" => Protocol::Udp,
            "tcp" => Protocol::Tcp,
            _ => {
                return Err(MappingRuleParseError::InvalidProtocol(
                    line.to_string(),
                    parts[2].to_string(),
                ));
            }
        };
        Ok(MappingRuleEntry {
            listen,
            upstream,
            protocol,
        })
    }
}

#[derive(Debug)]
struct MappingRule {
    listen: String,
    upstreams: Vec<String>,
    protocol: Protocol,
}

async fn read_mapping_file() -> Result<Vec<MappingRule>, std::io::Error> {
    let exe_path = std::env::current_exe()?;
    let dir = exe_path.parent().unwrap();
    let mapping_path = dir.join("mapping.txt");
    let file = File::open(&mapping_path).await?;
    let mut reader = BufReader::new(file);
    let mut rules: HashMap<(String, Protocol), Vec<String>> = HashMap::new();
    let mut line = String::new();
    while reader.read_line(&mut line).await? != 0 {
        match MappingRuleEntry::parse(&line) {
            Ok(entry) => {
                rules
                    .entry((entry.listen, entry.protocol))
                    .or_default()
                    .push(entry.upstream);
            }
            Err(e) => match e {
                MappingRuleParseError::Empty => (),
                MappingRuleParseError::InvalidFormat(input) => {
                    eprintln!("Invalid format: \"{}\"", input.trim())
                }
                MappingRuleParseError::InvalidProtocol(input, protocol) => {
                    eprintln!("Invalid protocol: {protocol} in \"{}\"", input.trim())
                }
            },
        }
        line.clear();
    }
    Ok(rules
        .into_iter()
        .map(|((listen, protocol), upstreams)| MappingRule {
            listen,
            upstreams,
            protocol,
        })
        .collect())
}

async fn handle_tcp_connection(
    upstream_addr: &str,
    mut downstream: TcpStream,
) -> Result<(), std::io::Error> {
    let mut upstream = TcpStream::connect(upstream_addr).await?;
    tokio::io::copy_bidirectional(&mut downstream, &mut upstream).await?;
    Ok(())
}

async fn run_tcp_proxy(listen_addr: &str, upstreams: Vec<String>) -> Result<(), std::io::Error> {
    let listener = TcpListener::bind(listen_addr).await?;
    println!("TCP proxy listening on {} -> {:?}", listen_addr, upstreams);
    let current = Arc::new(AtomicUsize::new(0));
    let upstreams = Arc::new(upstreams);
    loop {
        let (downstream, _) = listener.accept().await?;
        let current = current.clone();
        let upstreams = upstreams.clone();
        tokio::spawn(async move {
            let idx = current.fetch_add(1, Ordering::Relaxed) % upstreams.len();
            let upstream_addr = upstreams[idx].clone();
            if let Err(e) = handle_tcp_connection(&upstream_addr, downstream).await {
                eprintln!("TCP proxy error: {}", e);
            }
        });
    }
}

#[derive(Debug, Clone)]
struct UdpProxyState {
    client_map: Arc<Mutex<HashMap<SocketAddr, (Arc<UdpSocket>, SocketAddr, Instant)>>>,
    upstreams: Vec<String>,
}

impl UdpProxyState {
    fn new(upstreams: Vec<String>) -> Self {
        UdpProxyState {
            client_map: Arc::new(Mutex::new(HashMap::new())),
            upstreams,
        }
    }

    async fn get_upstream_socket(
        &self,
        client_addr: SocketAddr,
    ) -> Result<(Arc<UdpSocket>, SocketAddr), std::io::Error> {
        let mut map = self.client_map.lock().await;

        // 清理过期连接
        let now = Instant::now();
        map.retain(|_, (_, _, last_used)| now.duration_since(*last_used) < Duration::from_secs(30));

        // 查找或创建socket
        if let Some((sock, upstream_addr, _)) = map.get(&client_addr) {
            return Ok((sock.clone(), *upstream_addr));
        }

        // 选择上游服务器
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        client_addr.ip().hash(&mut hasher);
        let idx = hasher.finish() as usize % self.upstreams.len();
        let upstream_addr = self.upstreams[idx].parse().map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("Invalid upstream address: {}", e),
            )
        })?;

        // 创建新socket
        let sock = Arc::new(UdpSocket::bind("0.0.0.0:0").await?);
        sock.connect(upstream_addr).await?;

        // 存储并返回
        let entry = (sock.clone(), upstream_addr, Instant::now());
        map.insert(client_addr, entry);
        Ok((sock, upstream_addr))
    }
}

async fn run_udp_proxy(listen_addr: &str, upstreams: Vec<String>) -> Result<(), std::io::Error> {
    let socket = Arc::new(UdpSocket::bind(listen_addr).await?);
    println!("UDP proxy listening on {} -> {:?}", listen_addr, upstreams);

    let state = Arc::new(UdpProxyState::new(upstreams));
    let mut buf = [0u8; 65536];

    loop {
        let (len, client_addr) = socket.recv_from(&mut buf).await?;
        let data = buf[..len].to_vec();
        let socket_clone = socket.clone();
        let state_clone = state.clone();

        tokio::spawn(async move {
            // 获取或创建专用socket
            let (upstream_sock, upstream_addr) =
                match state_clone.get_upstream_socket(client_addr).await {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("Failed to get upstream socket: {}", e);
                        return;
                    }
                };

            // 发送到上游
            if let Err(e) = upstream_sock.send(&data).await {
                eprintln!("Send to {} failed: {}", upstream_addr, e);
                return;
            }

            // 接收响应（带超时和多包支持）
            let mut total_responses = 0;
            let start_time = Instant::now();
            while start_time.elapsed() < Duration::from_secs(5) {
                let mut resp_buf = [0u8; 65536];
                let timeout = tokio::time::sleep(Duration::from_millis(500));
                tokio::pin!(timeout);

                tokio::select! {
                    result = upstream_sock.recv(&mut resp_buf) => {
                        match result {
                            Ok(len) => {
                                total_responses += 1;
                                if let Err(e) = socket_clone.send_to(&resp_buf[..len], client_addr).await {
                                    eprintln!("Send to client {} failed: {}", client_addr, e);
                                }
                            }
                            Err(e) => {
                                eprintln!("Receive from {} failed: {}", upstream_addr, e);
                                break;
                            }
                        }
                    }
                    _ = &mut timeout => {
                        // 超时后检查是否需要继续等待
                        if total_responses > 0 {
                            // 至少收到一个响应，认为完成
                            break;
                        }
                    }
                }
            }
        });
    }
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let rules = read_mapping_file().await?;
    let mut handles = vec![];
    for rule in rules {
        match rule.protocol {
            Protocol::Tcp => {
                handles.push(tokio::spawn(async move {
                    if let Err(e) = run_tcp_proxy(&rule.listen, rule.upstreams).await {
                        eprintln!("TCP proxy failed: {}", e);
                    }
                }));
            }
            Protocol::Udp => {
                handles.push(tokio::spawn(async move {
                    if let Err(e) = run_udp_proxy(&rule.listen, rule.upstreams).await {
                        eprintln!("UDP proxy failed: {}", e);
                    }
                }));
            }
        }
    }
    for handle in handles {
        if let Err(e) = handle.await {
            eprintln!("Proxy failed: {}", e);
        }
    }
    Ok(())
}

use async_trait::async_trait;
use pingora::prelude::*;
use std::{collections::HashMap, io, sync::Arc};
use tokio::{
    fs::File,
    io::{AsyncBufReadExt, BufReader},
};

#[derive(Debug)]
struct MappingRuleEntry {
    /// 监听地址，如 ":80" (等于 "0.0.0.0:80") 或 "127.0.0.1:90"
    listen: String,
    /// 上游服务器
    upstream: String,
}

impl MappingRuleEntry {
    fn parse(line: &str) -> Option<Self> {
        let parts = line.split('#').next();
        if let Some(parts) = parts {
            let parts: Vec<&str> = parts.splitn(2, "->").map(|s| s.trim()).collect();
            if parts.len() != 2 {
                return None;
            }
            let listen = if parts[0].starts_with(':') {
                "0.0.0.0".to_string() + parts[0]
            } else {
                parts[0].to_string()
            };
            let upstream = if parts[1].starts_with(':') {
                "127.0.0.1".to_string() + parts[1]
            } else {
                parts[1].to_string()
            };
            Some(MappingRuleEntry { listen, upstream })
        } else {
            None
        }
    }
}

#[derive(Debug)]
struct MappingRule {
    /// 监听地址，如 ":80" (等于 "0.0.0.0:80") 或 "127.0.0.1:90"
    listen: String,
    /// 上游服务器列表
    upstreams: Vec<String>,
}

// 读取并解析 mapping.txt
async fn read_mapping_file() -> Result<Vec<MappingRule>, io::Error> {
    let exe_path = std::env::current_exe()?;
    let dir = exe_path.parent().expect("Failed to get parent directory");
    let mapping_path = dir.join("mapping.txt");
    let file = File::open(&mapping_path).await?;
    let mut reader = BufReader::new(file);
    let mut rules: HashMap<String, Vec<String>> = HashMap::new();
    let mut line = String::new();
    while let Ok(len) = reader.read_line(&mut line).await {
        if len == 0 {
            break;
        }
        if let Some(rule) = MappingRuleEntry::parse(&line) {
            rules.entry(rule.listen).or_default().push(rule.upstream);
        }
    }
    Ok(rules
        .into_iter()
        .map(|(listen, upstreams)| MappingRule { listen, upstreams })
        .collect())
}

pub struct LB(Arc<LoadBalancer<RoundRobin>>);
#[async_trait]
impl ProxyHttp for LB {
    type CTX = Option<String>;
    fn new_ctx(&self) -> Self::CTX {
        None
    }

    async fn upstream_peer(
        &self,
        _session: &mut Session,
        ctx: &mut Self::CTX,
    ) -> Result<Box<HttpPeer>> {
        let upstream = self.0.select(b"", 256).unwrap();
        println!("upstream peer is: {upstream:?}");
        let upstream_addr = upstream.to_string();
        let sni = upstream_addr.split(':').next().unwrap_or("localhost");
        *ctx = Some(sni.to_string());
        let peer = Box::new(HttpPeer::new(upstream, true, sni.to_string()));
        Ok(peer)
    }

    async fn upstream_request_filter(
        &self,
        _session: &mut Session,
        upstream_request: &mut RequestHeader,
        ctx: &mut Self::CTX,
    ) -> Result<()> {
        if let Some(host) = ctx {
            upstream_request.insert_header("Host", &*host).unwrap();
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut my_server = Server::new(Some(Opt::parse_args())).unwrap();
    my_server.bootstrap();
    let rules = read_mapping_file()
        .await
        .expect("Failed to read mapping.txt");
    for rule in rules {
        let mut upstreams = LoadBalancer::try_from_iter(rule.upstreams.iter()).unwrap();
        let hc = TcpHealthCheck::new();
        upstreams.set_health_check(hc);
        upstreams.health_check_frequency = Some(std::time::Duration::from_secs(1));
        let background =
            background_service(&format!("health check for {}", rule.listen), upstreams);
        let upstreams = background.task();
        let mut lb = http_proxy_service(&my_server.configuration, LB(upstreams));
        lb.add_tcp(&rule.listen);
        my_server.add_service(background);
        my_server.add_service(lb);
        println!("Created mapping: {} -> {:?}", rule.listen, rule.upstreams);
    }
    my_server.run_forever();
}

use std::{collections::HashMap, ops::RangeInclusive};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};

#[derive(Debug)]
pub enum ProtocolRaw {
    Tcp,
    Udp,
    TcpUdp,
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum Protocol {
    Tcp,
    Udp,
}

#[derive(Debug)]
pub struct MappingRuleRaw<'a> {
    pub protocol: ProtocolRaw,
    pub listen_port: RangeInclusive<u16>,
    pub upstream_host: &'a str,
    pub upstream_port: RangeInclusive<u16>,
}

#[derive(Debug)]
pub enum MappingRuleParseError<'a> {
    Empty,
    MissingListenPort,
    MissingUpstream,
    MissingUpstreamPort,
    InvalidProtocol(String),
    InvalidListenPort(&'a str),
    InvalidListenPortRange(&'a str),
    InvalidUpstream(&'a str),
    InvalidUpstreamPort(&'a str),
    InvalidUpstreamPortRange(&'a str),
    UnmatchedPortRange(RangeInclusive<u16>, RangeInclusive<u16>),
}

impl<'a> MappingRuleRaw<'a> {
    pub fn parse(line: &'a str) -> Result<Self, MappingRuleParseError<'a>> {
        // Skip empty lines and comments
        let mut parts = line
            .split('#')
            .next()
            .ok_or(MappingRuleParseError::Empty)?
            .split_whitespace();
        // Parse protocol
        let protocol = match parts
            .next()
            .ok_or(MappingRuleParseError::Empty)?
            .to_lowercase()
            .as_str()
        {
            "udp" => ProtocolRaw::Udp,
            "tcp" => ProtocolRaw::Tcp,
            "t+u" => ProtocolRaw::TcpUdp,
            input => {
                return Err(MappingRuleParseError::InvalidProtocol(input.to_string()));
            }
        };
        // Check listen
        let listen = parts
            .next()
            .ok_or(MappingRuleParseError::MissingListenPort)?;
        // Check upstream
        let upstream = parts.next().ok_or(MappingRuleParseError::MissingUpstream)?;
        // Parse listen port
        let mut listen_parts = listen.splitn(2, '-');
        let listen_from: u16 = listen_parts
            .next()
            .ok_or(MappingRuleParseError::InvalidListenPort(listen))?
            .parse()
            .map_err(|_| MappingRuleParseError::InvalidListenPort(listen))?;
        let listen_to: u16 = listen_parts
            .next()
            .map(|s| s.parse())
            .unwrap_or(Ok(listen_from))
            .map_err(|_| MappingRuleParseError::InvalidListenPort(listen))?;
        if listen_from > listen_to {
            return Err(MappingRuleParseError::InvalidListenPortRange(listen));
        }
        // Parse upstream
        let mut upstream_parts = upstream.splitn(2, ':');
        let upstream_host = {
            let t = upstream_parts
                .next()
                .ok_or(MappingRuleParseError::InvalidUpstream(upstream))?;
            if t.is_empty() { "localhost" } else { t }
        };
        let mut upstream_port_parts = upstream_parts
            .next()
            .ok_or(MappingRuleParseError::MissingUpstreamPort)?
            .splitn(2, '-');
        let upstream_port_from: u16 = upstream_port_parts
            .next()
            .ok_or(MappingRuleParseError::InvalidUpstreamPort(upstream))?
            .parse()
            .map_err(|_| MappingRuleParseError::InvalidUpstreamPort(upstream))?;
        let upstream_port_to: u16 = upstream_port_parts
            .next()
            .map(|s| s.parse())
            .unwrap_or(Ok(upstream_port_from))
            .map_err(|_| MappingRuleParseError::InvalidUpstreamPort(upstream))?;
        if upstream_port_from > upstream_port_to {
            return Err(MappingRuleParseError::InvalidUpstreamPortRange(upstream));
        }
        let listen_port = listen_from..=listen_to;
        let upstream_port = upstream_port_from..=upstream_port_to;
        if upstream_port_to - upstream_port_from != listen_to - listen_from {
            return Err(MappingRuleParseError::UnmatchedPortRange(
                listen_port,
                upstream_port,
            ));
        }
        Ok(Self {
            protocol,
            listen_port,
            upstream_host,
            upstream_port,
        })
    }
}

#[derive(Debug)]
pub struct MappingRule {
    pub protocol: Protocol,
    pub listen: String,
    pub upstream: String,
}

pub async fn read_mapping_file<T: Unpin + AsyncRead>(
    mut reader: BufReader<T>,
) -> Result<Vec<MappingRule>, std::io::Error> {
    let mut rules: HashMap<(Protocol, u16), (String, u16)> = HashMap::new();
    let mut line = String::new();
    while reader.read_line(&mut line).await? != 0 {
        line = line.trim().to_string();
        match MappingRuleRaw::parse(&line) {
            Ok(entry) => {
                match entry.protocol {
                    ProtocolRaw::Tcp => {
                        let upstream_port_from = entry.upstream_port.start();
                        for (i, port) in entry.listen_port.enumerate() {
                            if rules.contains_key(&(Protocol::Tcp, port)) {
                                eprintln!("Tcp mapping: port {port} will be overwritten")
                            }
                            rules.insert(
                                (Protocol::Tcp, port),
                                (
                                    entry.upstream_host.to_string(),
                                    upstream_port_from + i as u16,
                                ),
                            );
                        }
                    }
                    ProtocolRaw::Udp => {
                        let upstream_port_from = entry.upstream_port.start();
                        for (i, port) in entry.listen_port.enumerate() {
                            if rules.contains_key(&(Protocol::Udp, port)) {
                                eprintln!("Udp mapping: port {port} will be overwritten")
                            }
                            rules.insert(
                                (Protocol::Udp, port),
                                (
                                    entry.upstream_host.to_string(),
                                    upstream_port_from + i as u16,
                                ),
                            );
                        }
                    }
                    ProtocolRaw::TcpUdp => {
                        let upstream_port_from = entry.upstream_port.start();
                        for (i, port) in entry.listen_port.enumerate() {
                            if rules.contains_key(&(Protocol::Tcp, port)) {
                                eprintln!("Tcp mapping: port {port} will be overwritten")
                            }
                            rules.insert(
                                (Protocol::Tcp, port),
                                (
                                    entry.upstream_host.to_string(),
                                    upstream_port_from + i as u16,
                                ),
                            );
                            if rules.contains_key(&(Protocol::Udp, port)) {
                                eprintln!("Udp mapping: port {port} will be overwritten")
                            }
                            rules.insert(
                                (Protocol::Udp, port),
                                (
                                    entry.upstream_host.to_string(),
                                    upstream_port_from + i as u16,
                                ),
                            );
                        }
                    }
                };
            }
            Err(e) => match e {
                MappingRuleParseError::Empty => (),
                MappingRuleParseError::MissingListenPort => println!("Missing listen port: {line}"),
                MappingRuleParseError::MissingUpstream => println!("Missing upstream: {line}"),
                MappingRuleParseError::MissingUpstreamPort => {
                    println!("Missing upstream port: {line}")
                }
                MappingRuleParseError::InvalidProtocol(protocol) => {
                    println!("Invalid protocol: {protocol} in {line}")
                }
                MappingRuleParseError::InvalidListenPort(port) => {
                    println!("Invalid listen port: {port} in {line}")
                }
                MappingRuleParseError::InvalidListenPortRange(range) => {
                    println!("Invalid listen port range: {range} in {line}")
                }
                MappingRuleParseError::InvalidUpstream(upstream) => {
                    println!("Invalid upstream: {upstream} in {line}")
                }
                MappingRuleParseError::InvalidUpstreamPort(port) => {
                    println!("Invalid upstream port: {port} in {line}")
                }
                MappingRuleParseError::InvalidUpstreamPortRange(range) => {
                    println!("Invalid upstream port range: {range} in {line}")
                }
                MappingRuleParseError::UnmatchedPortRange(
                    listen_port_range,
                    upstream_port_range,
                ) => {
                    println!(
                        "Unmatched port range: {}-{} -> {}-{} in {line}",
                        listen_port_range.start(),
                        listen_port_range.end(),
                        upstream_port_range.start(),
                        upstream_port_range.end()
                    )
                }
            },
        }
        line.clear();
    }
    Ok(rules
        .into_iter()
        .map(
            |((protocol, listen), (upstream_host, upstream_port))| MappingRule {
                protocol,
                listen: format!("0.0.0.0:{listen}"),
                upstream: format!("{upstream_host}:{upstream_port}"),
            },
        )
        .collect())
}

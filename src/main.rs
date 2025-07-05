use port_mapping::{
    mapping_rule::{Protocol, read_mapping_file},
    tcp_proxy::TcpProxy,
    udp_proxy::UdpProxy,
};
use socket2::{Domain, Socket, Type};
use std::sync::Arc;
use tokio::{fs::File, io::BufReader};

fn get_udp_buffer_sizes() -> std::io::Result<usize> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, None)?;
    Ok(socket.recv_buffer_size()?)
}

async fn get_mapping_file() -> Result<BufReader<File>, std::io::Error> {
    let file = File::open("mapping.txt").await;
    Ok(match file {
        Ok(file) => BufReader::new(file),
        Err(_) => {
            let exe_path = std::env::current_exe()?;
            let dir = exe_path.parent().unwrap();
            let mapping_path = dir.join("mapping.txt");
            let file = File::open(&mapping_path).await?;
            BufReader::new(file)
        }
    })
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let reader = get_mapping_file().await?;
    let rules = read_mapping_file(reader).await?;
    let mut handles = vec![];
    let udp_buffer_size = get_udp_buffer_sizes()?;
    for rule in rules {
        match rule.protocol {
            Protocol::Tcp => {
                handles.push(tokio::spawn(async move {
                    let proxy = Arc::new(TcpProxy::new(rule.listen.clone(), rule.upstream.clone()));
                    if let Err(e) = proxy.run().await {
                        eprintln!("[warning][tcp] {} failed: {e}", rule.listen);
                    }
                }));
            }
            Protocol::Udp => {
                handles.push(tokio::spawn(async move {
                    let proxy = Arc::new(UdpProxy::new(
                        rule.listen.clone(),
                        rule.upstream.clone(),
                        udp_buffer_size,
                    ));
                    if let Err(e) = proxy.run().await {
                        eprintln!("[warning][tcp] {} failed: {e}", rule.listen);
                    }
                }));
            }
        }
    }
    for handle in handles {
        if let Err(e) = handle.await {
            eprintln!("[warning][thread] Failed: {e}");
        }
    }
    Ok(())
}

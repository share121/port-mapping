use port_mapping::{
    mapping_rule::{Protocol, read_mapping_file},
    tcp_proxy::TcpProxy,
};
use std::sync::Arc;
use tokio::{fs::File, io::BufReader};

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let exe_path = std::env::current_exe()?;
    let dir = exe_path.parent().unwrap();
    let mapping_path = dir.join("mapping.txt");
    let file = File::open(&mapping_path).await?;
    let reader = BufReader::new(file);
    let rules = read_mapping_file(reader).await?;
    let mut handles = vec![];
    for rule in rules {
        match rule.protocol {
            Protocol::Tcp => {
                handles.push(tokio::spawn(async move {
                    let proxy = Arc::new(TcpProxy::new(rule.listen.clone(), rule.upstream.clone()));
                    if let Err(e) = proxy.run().await {
                        eprintln!("TCP proxy failed: {}", e);
                    }
                }));
            }
            Protocol::Udp => {}
        }
    }
    for handle in handles {
        if let Err(e) = handle.await {
            eprintln!("Proxy failed: {}", e);
        }
    }
    Ok(())
}

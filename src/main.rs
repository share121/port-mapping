use port_mapping::{
    get_mapping_file, get_udp_buffer_sizes,
    mapping_rule::{Protocol, read_mapping_file},
    tcp_proxy::TcpProxy,
    udp_proxy::UdpProxy,
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let udp_buffer_size = get_udp_buffer_sizes()?;
    let reader = get_mapping_file().await?;
    let rules = read_mapping_file(reader).await?;
    let mut handles = vec![];
    for rule in rules {
        match rule.protocol {
            Protocol::Tcp => {
                handles.push(tokio::spawn(async move {
                    let proxy = Arc::new(TcpProxy::new(rule.listen.clone(), rule.upstream.clone()));
                    if let Err(e) = proxy.run().await {
                        eprintln!("[warning][tcp][{rule}] Failed: {e}");
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
                        eprintln!("[warning][tcp][{rule}] Failed: {e}");
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

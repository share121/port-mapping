use socket2::{Domain, Socket, Type};
use tokio::{fs::File, io::BufReader};

pub mod mapping_rule;
pub mod tcp_proxy;
pub mod udp_proxy;

pub fn get_udp_buffer_sizes() -> std::io::Result<usize> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, None)?;
    Ok(socket.recv_buffer_size()?)
}

pub async fn get_mapping_file() -> std::io::Result<BufReader<File>> {
    let file = File::open("mapping.txt").await;
    Ok(BufReader::new(match file {
        Ok(file) => file,
        Err(_) => {
            let exe_path = std::env::current_exe()?;
            let dir = exe_path.parent().unwrap();
            let mapping_path = dir.join("mapping.txt");
            File::open(&mapping_path).await?
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_udp_buffer_sizes() {
        let result = get_udp_buffer_sizes();
        assert!(result.is_ok());
        let buffer_size = result.unwrap();
        assert!(buffer_size > 0);
    }
}

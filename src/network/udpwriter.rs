use std::net::{IpAddr, Ipv6Addr, ToSocketAddrs, UdpSocket};

use crate::alc::pkt::PktWriter;
use crate::tools::error::Result;

pub struct UdpWriter<A>
where
    A: ToSocketAddrs,
{
    sock: UdpSocket,
    addr: A,
}

impl<A> UdpWriter<A>
where
    A: ToSocketAddrs,
{
    pub fn new(addr: A) -> Result<UdpWriter<A>> {
        let socket_addr: Vec<std::net::SocketAddr> = addr.to_socket_addrs()?.collect();
        let sock = UdpSocket::bind("0.0.0.0:0")?;
        sock.connect(socket_addr.as_slice())?;
        let writer = UdpWriter { sock, addr };
        writer.join_multicast()?;
        Ok(writer)
    }

    pub fn multicast_loop_v4(&self) -> Result<bool> {
        let success = self.sock.multicast_loop_v4()?;
        Ok(success)
    }

    pub fn multicast_loop_v6(&self) -> Result<bool> {
        let success = self.sock.multicast_loop_v6()?;
        Ok(success)
    }

    fn join_multicast(&self) -> Result<()> {
        let socket_addrs = self.addr.to_socket_addrs()?;
        for socket_addr in socket_addrs {
            let ip_addr = socket_addr.ip();
            if ip_addr.is_multicast() {
                match &ip_addr {
                    IpAddr::V4(addr) => {
                        self.sock
                            .join_multicast_v4(addr, &std::net::Ipv4Addr::UNSPECIFIED)?;
                    }
                    IpAddr::V6(addr) => {
                        self.sock.join_multicast_v6(addr, 0)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn leave_multicast(&self) -> Result<()> {
        let socket_addrs = self.addr.to_socket_addrs()?;
        for socket_addr in socket_addrs {
            let ip_addr = socket_addr.ip();
            if ip_addr.is_multicast() {
                match &ip_addr {
                    IpAddr::V4(addr) => {
                        self.sock
                            .leave_multicast_v4(addr, &std::net::Ipv4Addr::UNSPECIFIED)?;
                    }
                    IpAddr::V6(addr) => {
                        self.sock.leave_multicast_v6(addr, 0)?;
                    }
                }
            }
        }
        Ok(())
    }
}

impl<A> Drop for UdpWriter<A>
where
    A: ToSocketAddrs,
{
    fn drop(&mut self) {
        log::info!("Leave multicast");
        self.leave_multicast().ok();
    }
}

impl<A> PktWriter for UdpWriter<A>
where
    A: ToSocketAddrs,
{
    fn write(&self, pkt: &Vec<u8>) -> Result<usize> {
        let ret = self.sock.send(pkt)?;
        Ok(ret)
    }
}

#[cfg(test)]
mod tests {
    use crate::alc::pkt::PktWriter;

    fn init() {
        std::env::set_var("RUST_LOG", "debug");
        env_logger::builder().is_test(true).init()
    }

    #[test]
    pub fn test_udp_writer() {
        init();
        let writer = super::UdpWriter::new("224.0.0.1:3400").unwrap();
        writer.multicast_loop_v4().unwrap();
        writer.write(&vec![0, 1, 2]).unwrap();
    }
}

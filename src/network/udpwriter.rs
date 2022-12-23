use crate::tools::error::Result;
use std::net::{IpAddr, ToSocketAddrs, UdpSocket};

#[derive(Debug)]
/// UDP/IP writer
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
    /// Return a new UDP/IP writer
    pub fn new(addr: A) -> Result<UdpWriter<A>> {
        let socket_addr: Vec<std::net::SocketAddr> = addr.to_socket_addrs()?.collect();
        let sock = UdpSocket::bind("0.0.0.0:0")?;
        sock.connect(socket_addr.as_slice())?;
        let writer = UdpWriter { sock, addr };
        writer.join_multicast()?;
        Ok(writer)
    }

    /// Gets the value of the `IP_MULTICAST_LOOP` option for this socket.
    pub fn multicast_loop_v4(&self) -> Result<bool> {
        let success = self.sock.multicast_loop_v4()?;
        Ok(success)
    }

    /// Gets the value of the `IPV6_MULTICAST_LOOP` option for this socket.
    pub fn multicast_loop_v6(&self) -> Result<bool> {
        let success = self.sock.multicast_loop_v6()?;
        Ok(success)
    }

    /// Write a packet to this socket
    pub fn write(&self, pkt: &Vec<u8>) -> Result<usize> {
        let ret = self.sock.send(pkt)?;
        Ok(ret)
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

#[cfg(test)]
mod tests {
    #[test]
    pub fn test_udp_writer() {
        crate::tests::init();
        let writer = super::UdpWriter::new("224.0.0.1:3400").unwrap();
        writer.multicast_loop_v4().unwrap();
        writer.write(&vec![0, 1, 2]).unwrap();
    }
}

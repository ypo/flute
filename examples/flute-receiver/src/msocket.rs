use std::{
    net::Ipv4Addr,
    os::fd::{AsRawFd, FromRawFd},
    str::FromStr,
};

use pnet::util::Octets;

use libc::{
    ip_mreq_source as IpMreqSource, IPPROTO_IP, IP_ADD_SOURCE_MEMBERSHIP, IP_DROP_SOURCE_MEMBERSHIP,
};

const fn to_in_addr(addr: &Ipv4Addr) -> libc::in_addr {
    libc::in_addr {
        s_addr: u32::from_ne_bytes(addr.octets()),
    }
}

fn get_errno() -> i32 {
    unsafe { *libc::__errno_location() }
}

fn get_error_msg(errno_value: i32) -> Option<String> {
    let error_message = unsafe { libc::strerror(errno_value) };
    if error_message.is_null() {
        return None;
    }
    let c_str = unsafe { std::ffi::CStr::from_ptr(error_message) };
    Some(c_str.to_string_lossy().into_owned())
}

pub struct MSocket {
    pub sock: std::net::UdpSocket,
    source_addr: Option<Ipv4Addr>,
    group_addr: Ipv4Addr,
    interface: Ipv4Addr,
}

impl MSocket {
    pub fn new(
        endpoint: &flute::core::UDPEndpoint,
        eth: Option<&str>,
        nonblocking: bool,
    ) -> std::io::Result<Self> {
        log::info!("Create new Multicast Socket endpoint to {:?}", endpoint);

        let group_addr = match Ipv4Addr::from_str(&endpoint.destination_group_address) {
            Ok(res) => res,
            Err(_) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!(
                        "Fail to parse ip addr {}",
                        endpoint.destination_group_address
                    ),
                ))
            }
        };

        let socket_fd = unsafe { libc::socket(libc::AF_INET, libc::SOCK_DGRAM, 0) };
        if socket_fd == -1 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Fail to create UDP socket",
            ));
        }

        Self::set_reuse_address(socket_fd, true)?;
        Self::set_reuse_port(socket_fd, true)?;
        Self::set_receive_buffer_size(socket_fd, 1024 * 1024)?;
        Self::bind_socket(socket_fd, &group_addr, endpoint.port)?;

        let sock = unsafe { std::net::UdpSocket::from_raw_fd(socket_fd) };
        sock.set_nonblocking(nonblocking)?;

        let interface = match eth {
            Some(res) => Ipv4Addr::from_str(res)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?,
            None => Ipv4Addr::UNSPECIFIED,
        };

        let source_addr = match &endpoint.source_address {
            Some(res) => Some(
                Ipv4Addr::from_str(res)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?,
            ),
            None => None,
        };

        if source_addr.is_some() && Self::is_ssm_addr(&group_addr) {
            Self::join_ssm(
                socket_fd,
                source_addr.as_ref().unwrap(),
                &group_addr,
                &interface,
            )?;
        } else {
            log::info!("Join multicast on interface {}", interface);
            sock.join_multicast_v4(&group_addr, &interface)?;
        }

        Ok(MSocket {
            sock,
            source_addr,
            group_addr,
            interface,
        })
    }

    fn is_ssm_addr(group_addr: &Ipv4Addr) -> bool {
        group_addr.octets()[0] == 232
    }

    fn bind_socket(socket_fd: i32, address: &Ipv4Addr, port: u16) -> std::io::Result<()> {
        let sockaddr = libc::sockaddr_in {
            sin_family: libc::AF_INET as u16,
            sin_port: u16::from_ne_bytes(port.octets()),
            sin_addr: libc::in_addr {
                s_addr: u32::from_ne_bytes(address.octets()),
            },
            sin_zero: [0; 8],
        };

        let sockaddr_ptr = &sockaddr as *const libc::sockaddr_in as *const libc::sockaddr;
        let sockaddr_len = std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t;

        let ret = unsafe { libc::bind(socket_fd, sockaddr_ptr, sockaddr_len) };

        if ret == -1 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Fail to bind socket {:?}", get_error_msg(get_errno())),
            ));
        }

        Ok(())
    }

    fn join_ssm(
        sock: i32,
        source: &Ipv4Addr,
        group: &Ipv4Addr,
        interface: &Ipv4Addr,
    ) -> std::io::Result<()> {
        log::debug!("Join SSM {} {} {}", source, group, interface);
        let mreqs = IpMreqSource {
            imr_multiaddr: to_in_addr(group),
            imr_interface: to_in_addr(interface),
            imr_sourceaddr: to_in_addr(source),
        };
        Self::setsockopt(sock, IPPROTO_IP, IP_ADD_SOURCE_MEMBERSHIP, mreqs)
    }

    fn leave_ssm(
        sock: i32,
        source: &Ipv4Addr,
        group: &Ipv4Addr,
        interface: &Ipv4Addr,
    ) -> std::io::Result<()> {
        log::debug!("Leave SSM {} {} {}", source, group, interface);
        let mreqs = IpMreqSource {
            imr_multiaddr: to_in_addr(group),
            imr_interface: to_in_addr(interface),
            imr_sourceaddr: to_in_addr(source),
        };
        Self::setsockopt(sock, IPPROTO_IP, IP_DROP_SOURCE_MEMBERSHIP, mreqs)
    }

    fn set_reuse_address(sock: i32, reuse: bool) -> std::io::Result<()> {
        Self::setsockopt(
            sock,
            libc::SOL_SOCKET,
            libc::SO_REUSEADDR,
            match reuse {
                true => 1 as i32,
                false => 0 as i32,
            },
        )
    }

    fn set_reuse_port(sock: i32, reuse: bool) -> std::io::Result<()> {
        Self::setsockopt(
            sock,
            libc::SOL_SOCKET,
            libc::SO_REUSEPORT,
            match reuse {
                true => 1 as i32,
                false => 0 as i32,
            },
        )
    }

    fn set_receive_buffer_size(sock: i32, size: usize) -> std::io::Result<()> {
        Self::setsockopt(sock, libc::SOL_SOCKET, libc::SO_RCVBUF, size)
    }

    fn setsockopt<T>(
        sock: libc::c_int,
        level: libc::c_int,
        name: libc::c_int,
        data: T,
    ) -> std::io::Result<()> {
        let data_ptr: *const libc::c_void = &data as *const _ as *const libc::c_void;
        let ret = unsafe {
            libc::setsockopt(
                sock as libc::c_int,
                level,
                name,
                data_ptr,
                std::mem::size_of::<T>() as libc::socklen_t,
            )
        };
        match ret {
            0 => Ok(()),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!(
                    "Fail to set opt {} errno={:?}",
                    ret,
                    get_error_msg(get_errno())
                ),
            )),
        }
    }
}

impl Drop for MSocket {
    fn drop(&mut self) {
        let fd = self.sock.as_raw_fd();
        if self.source_addr.is_some() && Self::is_ssm_addr(&self.group_addr) {
            Self::leave_ssm(
                fd,
                self.source_addr.as_ref().unwrap(),
                &self.group_addr,
                &self.interface,
            )
            .ok();
        } else {
            log::info!("Leave Multicast V4 on interface {}", self.interface);
            self.sock
                .leave_multicast_v4(&self.group_addr, &self.interface)
                .ok();
        }
    }
}

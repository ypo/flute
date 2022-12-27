//! # FLUTE - File Delivery over Unidirectional Transport
//!
//! Massively scalable multicast distribution solution
//!
//! The library implements a unidirectional file delivery (return channel is not required).
//!
//!
//! # RFC
//!
//! RFC used to implement this library
//!
//! | RFC      | Title      | Link       |
//! | ------------- | ------------- | ------------- |
//! | RFC 6726 | FLUTE - File Delivery over Unidirectional Transport | <https://www.rfc-editor.org/rfc/rfc6726.html> |
//! | RFC 5775 | Asynchronous Layered Coding (ALC) Protocol Instantiation | <https://www.rfc-editor.org/rfc/rfc5775.html> |
//! | RFC 5052 | Forward Error Correction (FEC) Building Block | <https://www.rfc-editor.org/rfc/rfc5052> |
//! | RFC 5510 | Reed-Solomon Forward Error Correction (FEC) Schemes | <https://www.rfc-editor.org/rfc/rfc5510.html> |
//!
//! # UDP/IP Multicast files sender
//!```rust
//! use flute::sender::Sender;
//! use flute::sender::ObjectDesc;
//! use flute::sender::CENC;
//! use std::net::UdpSocket;
//!
//! // Create UDP Socket
//! let udp_socket = UdpSocket::bind("0.0.0.0:0").unwrap();
//! udp_socket.connect("224.0.0.1:3400").expect("Connection failed");
//!
//! // Create FLUTE Sender
//! let tsi = 1;
//! let fdtid = 1;
//! let mut sender = Sender::new(tsi, fdtid, &Default::default(), CENC::Null);
//!
//! // Add object(s) (files) to the FLUTE sender
//! let obj = ObjectDesc::create_from_buffer(b"hello world", "text/plain",
//! &url::Url::parse("file:///hello.txt").unwrap(), 1, None, CENC::Null, true, None, true).unwrap();
//! sender.add_object(obj);
//!
//! // Always call publish after adding objects
//! sender.publish();
//!
//! // Send FLUTE packets over UDP/IP
//! while let Some(pkt) = sender.read() {
//!     udp_socket.send(&pkt).unwrap();
//!     std::thread::sleep(std::time::Duration::from_millis(1));
//! }
//!
//!```
//! # UDP/IP Multicast files receiver
//!```
//! use flute::receiver::{objectwriter, MultiReceiver};
//! use std::net::UdpSocket;
//!
//! // Create UDP/IP socket to receive FLUTE pkt
//! let udp_socket = UdpSocket::bind("224.0.0.1:3400").expect("Fail to bind");
//! 
//! // Create a writer able to write received files to the filesystem
//! let writer = objectwriter::FluteWriterFS::new(&std::path::Path::new("./flute_dir"))
//!     .unwrap_or_else(|_| std::process::exit(0));
//! 
//! // Create a multi-receiver capable of de-multiplexing several FLUTE sessions
//! let mut receiver = MultiReceiver::new(None, writer, None);
//!
//! // Receive pkt from UDP/IP socket and push it to the FLUTE receiver
//! let mut buf = [0; 2048];
//! loop {
//!     let (n, _src) = udp_socket.recv_from(&mut buf).expect("Failed to receive data");
//!     receiver.push(&buf[..n]).unwrap();
//!     receiver.cleanup();
//! }
//!```

#![deny(missing_docs)]
#![deny(missing_debug_implementations)]
#![cfg_attr(test, deny(warnings))]

mod alc;
mod fec;
mod tools;

///
/// FLUTE Receivers to re-construct ALC/LCT packets to Objects (files)
///
pub mod receiver {
    pub use crate::alc::multireceiver::MultiReceiver;
    pub use crate::alc::objectwriter;
    pub use crate::alc::receiver::Config;
    pub use crate::alc::receiver::Receiver;
}

/// FLUTE Sender to convert Objects (files) to ALC/LCT packets
pub mod sender {
    pub use crate::alc::lct::CENC;
    pub use crate::alc::objectdesc::ObjectDesc;
    pub use crate::alc::oti::FECEncodingID;
    pub use crate::alc::oti::Oti;
    pub use crate::alc::sender::Sender;
}

pub use crate::tools::error;

#[cfg(test)]
mod tests {
    pub fn init() {
        std::env::set_var("RUST_LOG", "debug");
        env_logger::builder().is_test(true).try_init().ok();
    }
}

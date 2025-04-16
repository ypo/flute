//! [![Rust](https://github.com/ypo/flute/actions/workflows/rust.yml/badge.svg)](https://github.com/ypo/flute/actions/workflows/rust.yml)
//! [![Python](https://github.com/ypo/flute/actions/workflows/python.yml/badge.svg)](https://github.com/ypo/flute/actions/workflows/python.yml)
//! [![Docs.rs](https://docs.rs/flute/badge.svg)](https://docs.rs/crate/flute/)
//! [![Crates.io](https://img.shields.io/crates/v/flute)](https://crates.io/crates/flute/)
//! [![Rust Dependency](https://deps.rs/repo/github/ypo/flute/status.svg)](https://deps.rs/repo/github/ypo/flute)
//! [![codecov](https://codecov.io/gh/ypo/flute/branch/main/graph/badge.svg?token=P4KE639YU8)](https://codecov.io/gh/ypo/flute)
//!
//! # FLUTE - File Delivery over Unidirectional Transport
//!
//!
//! Massively scalable multicast distribution solution
//!
//! The library implements a unidirectional file delivery, without the need of a return channel.
//!
//!
//! # RFC
//!
//! This library implements the following RFCs
//!
//!| RFC      | Title      | Link       |
//!| -------- | ---------- | -----------|
//!| RFC 6726 | FLUTE - File Delivery over Unidirectional Transport      | <https://www.rfc-editor.org/rfc/rfc6726.html> |
//!| RFC 5775 | Asynchronous Layered Coding (ALC) Protocol Instantiation | <https://www.rfc-editor.org/rfc/rfc5775.html> |
//!| RFC 5661 | Layered Coding Transport (LCT) Building Block            | <https://www.rfc-editor.org/rfc/rfc5651>      |
//!| RFC 5052 | Forward Error Correction (FEC) Building Block            | <https://www.rfc-editor.org/rfc/rfc5052>      |
//!| RFC 5510 | Reed-Solomon Forward Error Correction (FEC) Schemes      | <https://www.rfc-editor.org/rfc/rfc5510.html> |
//!| 3GPP TS 26.346 | Extended FLUTE FDT Schema (7.2.10)      | <https://www.etsi.org/deliver/etsi_ts/126300_126399/126346/17.03.00_60/ts_126346v170300p.pdf> |
//!
//! # Thread Safety
//! 
//! ## FLUTE Sender
//! 
//! The FLUTE Sender is designed to be safely shared between multiple threads.
//! 
//! ## FLUTE Receiver and Tokio Integration
//! 
//! Unlike the sender, the FLUTE Receiver **is not thread-safe** and cannot be shared between multiple threads. 
//! To integrate it with Tokio, you must use `tokio::task::LocalSet`, which allows spawning tasks that require a single-threaded runtime. 
//! 
//! The following example demonstrates how to use the FLUTE Receiver with Tokio:
//!
//!```ignore
//! use flute::receiver::{writer, MultiReceiver};
//! use std::rc::Rc;
//! 
//!#[tokio::main]
//!async fn main() {
//!    let local = task::LocalSet::new();
//!    // Run the local task set.
//!    local.run_until(async move {
//!        let nonsend_data = nonsend_data.clone();
//!        task::spawn_local(async move {
//!            let writer = Rc::new(writer::ObjectWriterFSBuilder::new(&std::path::Path::new("./flute_dir"), true).unwrap_or_else(|_| std::process::exit(0)));
//!            let mut receiver = MultiReceiver::new(writer, None, false);
//!            // ... run the receiver
//!        }).await.unwrap();
//!    }).await;
//!}
//!```
//! # UDP/IP Multicast files sender
//!
//! Transfer files over a UDP/IP network
//!
//!```rust
//! use flute::sender::Sender;
//! use flute::sender::ObjectDesc;
//! use flute::core::lct::Cenc;
//! use flute::core::UDPEndpoint;
//! use std::net::UdpSocket;
//! use std::time::SystemTime;
//!
//! // Create UDP Socket
//! let udp_socket = UdpSocket::bind("0.0.0.0:0").unwrap();
//! udp_socket.connect("224.0.0.1:3400").expect("Connection failed");
//!
//! // Create FLUTE Sender
//! let tsi = 1;
//! let oti = Default::default();
//! let config = Default::default();
//! let endpoint = UDPEndpoint::new(None, "224.0.0.1".to_string(), 3400);
//! let mut sender = Sender::new(endpoint, tsi, &oti, &config);
//!
//! // Add object(s) (files) to the FLUTE sender (priority queue 0)
//! let obj = ObjectDesc::create_from_buffer(b"hello world".to_vec(), "text/plain",
//! &url::Url::parse("file:///hello.txt").unwrap(), 1, None, None, None, None, Cenc::Null, true, None, true).unwrap();
//! sender.add_object(0, obj);
//!
//! // Always call publish after adding objects
//! sender.publish(SystemTime::now());
//!
//! // Send FLUTE packets over UDP/IP
//! while let Some(pkt) = sender.read(SystemTime::now()) {
//!     udp_socket.send(&pkt).unwrap();
//!     std::thread::sleep(std::time::Duration::from_millis(1));
//! }
//!
//!```
//! # UDP/IP Multicast files receiver
//!
//! Receive files from a UDP/IP network
//!
//!```
//! use flute::receiver::{writer, MultiReceiver};
//! use flute::core::UDPEndpoint;
//! use std::net::UdpSocket;
//! use std::time::SystemTime;
//! use std::rc::Rc;
//!
//! // Create UDP/IP socket to receive FLUTE pkt
//! let endpoint = UDPEndpoint::new(None, "224.0.0.1".to_string(), 3400);
//! let udp_socket = UdpSocket::bind(format!("{}:{}", endpoint.destination_group_address, endpoint.port)).expect("Fail to bind");
//!
//! // Create a writer able to write received files to the filesystem
//! let enable_md5_check = true;
//! let writer = Rc::new(writer::ObjectWriterFSBuilder::new(&std::path::Path::new("./flute_dir"), enable_md5_check)
//!     .unwrap_or_else(|_| std::process::exit(0)));
//!
//! // Create a multi-receiver capable of de-multiplexing several FLUTE sessions
//! let mut receiver = MultiReceiver::new(writer, None, false);
//!
//! // Receive pkt from UDP/IP socket and push it to the FLUTE receiver
//! let mut buf = [0; 2048];
//! loop {
//!     let (n, _src) = udp_socket.recv_from(&mut buf).expect("Failed to receive data");
//!     let now = SystemTime::now();
//!     receiver.push(&endpoint, &buf[..n], now).unwrap();
//!     receiver.cleanup(now);
//! }
//!```
//! # Application-Level Forward Erasure Correction (AL-FEC)
//!
//! The following error recovery algorithms are supported
//!
//! - [X] No-code
//! - [X] Reed-Solomon GF 2^8  
//! - [X] Reed-Solomon GF 2^8 Under Specified
//! - [ ] Reed-Solomon GF 2^16  
//! - [ ] Reed-Solomon GF 2^m  
//! - [X] RaptorQ  
//! - [X] Raptor
//!
//! The `Oti` module provides an implementation of the Object Transmission Information (OTI)
//! used to configure Forward Error Correction (FEC) encoding in the FLUTE protocol.
//!
//!```rust
//! use flute::sender::Sender;
//! use flute::core::Oti;
//! use flute::core::UDPEndpoint;
//!
//! // Reed Solomon 2^8 with encoding blocks composed of  
//! // 60 source symbols and 4 repair symbols of 1424 bytes per symbol
//! let endpoint = UDPEndpoint::new(None, "224.0.0.1".to_string(), 3400);
//! let oti = Oti::new_reed_solomon_rs28(1424, 60, 4).unwrap();
//! let mut sender = Sender::new(endpoint, 1, &oti, &Default::default());
//!```
//!
//! # Content Encoding (CENC)
//!
//! The following schemes are supported during the transmission/reception
//!
//! - [x] Null (no compression)
//! - [x] Deflate
//! - [x] Zlib
//! - [x] Gzip
//!
//! # Files multiplex / Blocks interleave
//!
//! The FLUTE Sender is able to transfer multiple files in parallel by interleaving packets from each file. For example:
//!
//! **Pkt file1** -> Pkt file2 -> Pkt file3 -> **Pkt file1** -> Pkt file2 -> Pkt file3 ...
//!
//! The Sender can interleave blocks within a single file.  
//! The following example shows Encoding Symbols (ES) from different blocks (B) are interleaved. For example:  
//!
//! **(B 1,ES 1)**->(B 2,ES 1)->(B 3,ES 1)->**(B 1,ES 2)**->(B 2,ES 2)...
//!
//! To configure the multiplexing, use the `Config` struct as follows:
//!
//!```rust
//! use flute::sender::Sender;
//! use flute::sender::Config;
//! use flute::sender::PriorityQueue;
//! use flute::core::UDPEndpoint;
//!
//! let mut config = Config {
//!     // Interleave a maximum of 3 blocks within each file
//!     interleave_blocks: 3,
//!     ..Default::default()
//! };
//!
//! // Interleave a maximum of 3 files in priority queue '0'
//! config.set_priority_queue(PriorityQueue::HIGHEST, PriorityQueue::new(3));
//!
//! let endpoint = UDPEndpoint::new(None, "224.0.0.1".to_string(), 3400);
//! let mut sender = Sender::new(endpoint, 1, &Default::default(), &config);
//!```
//!
//! # Priority Queues
//!
//! FLUTE sender can be configured with multiple queues, each having a different priority level.  
//! Files in higher priority queues are always transferred before files in lower priority queues.  
//! Transfer of files in lower priority queues is paused while there are files to be transferred in higher priority queues.  
//!
//!```rust
//! 
//! use flute::sender::Sender;
//! use flute::sender::Config;
//! use flute::sender::PriorityQueue;
//! use flute::core::UDPEndpoint;
//! use flute::sender::ObjectDesc;
//! use flute::core::lct::Cenc;
//!
//! // Create a default configuration
//! let mut config: flute::sender::Config = Default::default();
//!
//! // Configure the HIGHEST priority queue with a capacity of 3 simultaneous file transfer
//! config.set_priority_queue(PriorityQueue::HIGHEST, PriorityQueue::new(3));
//!
//! // Configure the LOW priority queue with a capacity of 1 file transfer at a time
//! config.set_priority_queue(PriorityQueue::LOW, PriorityQueue::new(1));
//!
//! let endpoint = UDPEndpoint::new(None, "224.0.0.1".to_string(), 3400);
//! let mut sender = Sender::new(endpoint, 1, &Default::default(), &config);
//!
//! // Create an ObjectDesc for a low priority file
//! let low_priority_obj = ObjectDesc::create_from_buffer(b"low priority".to_vec(), "text/plain",
//! &url::Url::parse("file:///low_priority.txt").unwrap(), 1, None, None, None, None, Cenc::Null, true, None, true).unwrap();
//!
//! // Create an ObjectDesc for a high priority file
//! let high_priority_obj = ObjectDesc::create_from_buffer(b"high priority".to_vec(), "text/plain",
//! &url::Url::parse("file:///high_priority.txt").unwrap(), 1, None, None, None, None, Cenc::Null, true, None, true).unwrap();
//!
//! // Put Object to the low priority queue
//! sender.add_object(PriorityQueue::LOW, low_priority_obj);
//!
//! // Put Object to the high priority queue
//! sender.add_object(PriorityQueue::HIGHEST, high_priority_obj);
//!```
//! 
//! # Bitrate Control
//!The FLUTE library does not provide a built-in bitrate control mechanism. 
//! Users are responsible for controlling the bitrate by sending packets at a specific rate. 
//! However, the library offers a way to control the target transfer duration or the target transfer end time for each file individually.
//!
//!To ensure proper functionality, the target transfer mechanism requires that the overall bitrate is sufficiently high.
//! 
//! ## Target Transfer Duration
//! 
//! The sender will attempt to transfer the file within the specified duration.
//! 
//!```rust
//! 
//! use flute::sender::Sender;
//! use flute::sender::ObjectDesc;
//! use flute::sender::TargetAcquisition;
//! use flute::core::lct::Cenc;
//! use flute::core::UDPEndpoint;
//! use std::net::UdpSocket;
//! use std::time::SystemTime;
//!
//! // Create UDP Socket
//! let udp_socket = UdpSocket::bind("0.0.0.0:0").unwrap();
//! udp_socket.connect("224.0.0.1:3400").expect("Connection failed");
//!
//! // Create FLUTE Sender
//! let tsi = 1;
//! let oti = Default::default();
//! let config = Default::default();
//! let endpoint = UDPEndpoint::new(None, "224.0.0.1".to_string(), 3400);
//! let mut sender = Sender::new(endpoint, tsi, &oti, &config);
//!
//! // Create an Object
//! let mut obj = ObjectDesc::create_from_buffer(b"hello world".to_vec(), "text/plain",
//! &url::Url::parse("file:///hello.txt").unwrap(), 1, None, None, None, None, Cenc::Null, true, None, true).unwrap();
//! 
//! // Set the Target Transfer Duration of this object to 2 seconds
//! obj.target_acquisition = Some(TargetAcquisition::WithinDuration(std::time::Duration::from_secs(2)));
//! 
//! // Add object(s) (files) to the FLUTE sender (priority queue 0)
//! sender.add_object(0, obj);
//!
//! // Always call publish after adding objects
//! sender.publish(SystemTime::now());
//!
//! // Send FLUTE packets over UDP/IP
//! while sender.nb_objects() > 0  {
//!     if let Some(pkt) = sender.read(SystemTime::now()) {
//!         udp_socket.send(&pkt).unwrap();
//!     } else {
//!        std::thread::sleep(std::time::Duration::from_millis(1));
//!     }
//! }
//!```
//! 
//! ## Target Time to End Transfer
//! 
//! The sender will try to finish the file at the specified time.
//! 
//!```rust
//! 
//! use flute::sender::Sender;
//! use flute::sender::ObjectDesc;
//! use flute::sender::TargetAcquisition;
//! use flute::core::lct::Cenc;
//! use flute::core::UDPEndpoint;
//! use std::net::UdpSocket;
//! use std::time::SystemTime;
//!
//! // Create UDP Socket
//! let udp_socket = UdpSocket::bind("0.0.0.0:0").unwrap();
//! udp_socket.connect("224.0.0.1:3400").expect("Connection failed");
//!
//! // Create FLUTE Sender
//! let tsi = 1;
//! let oti = Default::default();
//! let config = Default::default();
//! let endpoint = UDPEndpoint::new(None, "224.0.0.1".to_string(), 3400);
//! let mut sender = Sender::new(endpoint, tsi, &oti, &config);
//!
//! // Create an Object
//! let mut obj = ObjectDesc::create_from_buffer(b"hello world".to_vec(), "text/plain",
//! &url::Url::parse("file:///hello.txt").unwrap(), 1, None, None, None, None, Cenc::Null, true, None, true).unwrap();
//! 
//! // Set the Target Transfer End Time of this object to 10 seconds from now
//! let target_end_time = SystemTime::now() + std::time::Duration::from_secs(10);
//! obj.target_acquisition = Some(TargetAcquisition::WithinTime(target_end_time));
//! 
//! // Add object(s) (files) to the FLUTE sender (priority queue 0)
//! sender.add_object(0, obj);
//!
//! // Always call publish after adding objects
//! sender.publish(SystemTime::now());
//! 
//! // Send FLUTE packets over UDP/IP
//! while sender.nb_objects() > 0  {
//!     if let Some(pkt) = sender.read(SystemTime::now()) {
//!         udp_socket.send(&pkt).unwrap();
//!     } else {
//!        std::thread::sleep(std::time::Duration::from_millis(1));
//!     }
//! }
//!```

#![deny(missing_docs)]
#![deny(missing_debug_implementations)]
#![cfg_attr(test, deny(warnings))]

mod common;
mod fec;
mod tools;

pub mod receiver;
pub mod sender;
pub use crate::tools::error;

/// Core module with low-level function
pub mod core {

    /// ALC packets
    pub mod alc {
        pub use crate::common::alc::get_sender_current_time;
        pub use crate::common::alc::parse_alc_pkt;
        pub use crate::common::alc::parse_payload_id;
        pub use crate::common::alc::AlcPkt;
        pub use crate::common::alc::PayloadID;
    }

    /// LCT packets
    pub mod lct {
        pub use crate::common::lct::get_ext;
        pub use crate::common::lct::inc_hdr_len;
        pub use crate::common::lct::push_lct_header;
        pub use crate::common::lct::Cenc;
        pub use crate::common::lct::LCTHeader;
    }

    pub use crate::common::oti::FECEncodingID;
    pub use crate::common::oti::Oti;
    pub use crate::common::udpendpoint::UDPEndpoint;
}

#[cfg(feature = "python")]
mod py;

#[cfg(test)]
mod tests {
    pub fn init() {
        std::env::set_var("RUST_LOG", "debug");
        env_logger::builder().is_test(true).try_init().ok();
    }
}

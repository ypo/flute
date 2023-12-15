[![Rust](https://github.com/ypo/flute/actions/workflows/rust.yml/badge.svg)](https://github.com/ypo/flute/actions/workflows/rust.yml)
[![Python](https://github.com/ypo/flute/actions/workflows/python.yml/badge.svg)](https://github.com/ypo/flute/actions/workflows/python.yml)
[![Docs.rs](https://docs.rs/flute/badge.svg)](https://docs.rs/crate/flute/)
[![Crates.io](https://img.shields.io/crates/v/flute)](https://crates.io/crates/flute/)
[![Rust Dependency](https://deps.rs/repo/github/ypo/flute/status.svg)](https://deps.rs/repo/github/ypo/flute)
[![codecov](https://codecov.io/gh/ypo/flute/branch/main/graph/badge.svg?token=P4KE639YU8)](https://codecov.io/gh/ypo/flute)

## FLUTE - File Delivery over Unidirectional Transport


Massively scalable multicast distribution solution

The library implements a unidirectional file delivery, without the need of a return channel.


## RFC

This library implements the following RFCs

| RFC      | Title      | Link       |
| -------- | ---------- | -----------|
| RFC 6726 | FLUTE - File Delivery over Unidirectional Transport      | <https://www.rfc-editor.org/rfc/rfc6726.html> |
| RFC 5775 | Asynchronous Layered Coding (ALC) Protocol Instantiation | <https://www.rfc-editor.org/rfc/rfc5775.html> |
| RFC 5661 | Layered Coding Transport (LCT) Building Block            | <https://www.rfc-editor.org/rfc/rfc5651>      |
| RFC 5052 | Forward Error Correction (FEC) Building Block            | <https://www.rfc-editor.org/rfc/rfc5052>      |
| RFC 5510 | Reed-Solomon Forward Error Correction (FEC) Schemes      | <https://www.rfc-editor.org/rfc/rfc5510.html> |

## UDP/IP Multicast files sender

Transfer files over a UDP/IP network

```rust
use flute::sender::Sender;
use flute::sender::ObjectDesc;
use flute::sender::Cenc;
use flute::core::UDPEndpoint;
use std::net::UdpSocket;
use std::time::SystemTime;

// Create UDP Socket
let udp_socket = UdpSocket::bind("0.0.0.0:0").unwrap();
udp_socket.connect("224.0.0.1:3400").expect("Connection failed");

// Create FLUTE Sender
let tsi = 1;
let oti = Default::default();
let config = Default::default();
let endpoint = UDPEndpoint::new(None, "224.0.0.1".to_string(), 3400);
let mut sender = Sender::new(endpoint, tsi, &oti, &config);

// Add object(s) (files) to the FLUTE sender
let obj = ObjectDesc::create_from_buffer(b"hello world", "text/plain",
&url::Url::parse("file:///hello.txt").unwrap(), 1, None, None, None, Cenc::Null, true, None, true).unwrap();
sender.add_object(obj);

// Always call publish after adding objects
sender.publish(SystemTime::now());

// Send FLUTE packets over UDP/IP
while let Some(pkt) = sender.read(SystemTime::now()) {
    udp_socket.send(&pkt).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1));
}

```
## UDP/IP Multicast files receiver

Receive files from a UDP/IP network

```rust
use flute::receiver::{writer, MultiReceiver};
use flute::core::UDPEndpoint;
use std::net::UdpSocket;
use std::time::SystemTime;
use std::rc::Rc;

// Create UDP/IP socket to receive FLUTE pkt
let endpoint = UDPEndpoint::new(None, "224.0.0.1".to_string(), 3400);
let udp_socket = UdpSocket::bind(format!("{}:{}", endpoint.destination_group_address, endpoint.port)).expect("Fail to bind");

// Create a writer able to write received files to the filesystem
let writer = Rc::new(writer::ObjectWriterFSBuilder::new(&std::path::Path::new("./flute_dir"))
    .unwrap_or_else(|_| std::process::exit(0)));

// Create a multi-receiver capable of de-multiplexing several FLUTE sessions
let mut receiver = MultiReceiver::new(writer, None, false);

// Receive pkt from UDP/IP socket and push it to the FLUTE receiver
let mut buf = [0; 2048];
loop {
    let (n, _src) = udp_socket.recv_from(&mut buf).expect("Failed to receive data");
    let now = SystemTime::now();
    receiver.push(&endpoint, &buf[..n], now).unwrap();
    receiver.cleanup(now);
}
```
## Application-Level Forward Erasure Correction (AL-FEC)

The following error recovery algorithms are supported

- [X] No-code
- [X] Reed-Solomon GF 2^8
- [X] Reed-Solomon GF 2^8 Under Specified
- [ ] Reed-Solomon GF 2^16
- [ ] Reed-Solomon GF 2^m
- [X] RaptorQ
- [X] Raptor

The `Oti` module provides an implementation of the Object Transmission Information (OTI)
used to configure Forward Error Correction (FEC) encoding in the FLUTE protocol.

```rust
use flute::sender::Oti;
use flute::sender::Sender;
use flute::core::UDPEndpoint;

// Reed Solomon 2^8 with encoding blocks composed of
// 60 source symbols and 4 repair symbols of 1424 bytes per symbol
let endpoint = UDPEndpoint::new(None, "224.0.0.1".to_string(), 3400);
let oti = Oti::new_reed_solomon_rs28(1424, 60, 4).unwrap();
let mut sender = Sender::new(endpoint, 1, &oti, &Default::default());
```

## Content Encoding (CENC)

The following schemes are supported during the transmission/reception

- [x] Null (no compression)
- [x] Deflate
- [x] Zlib
- [x] Gzip

## Files multiplex / Blocks interleave

The FLUTE Sender is able to transfer multiple files in parallel by interleaving packets from each file. For example:

**Pkt file1** -> Pkt file2 -> Pkt file3 -> **Pkt file1** -> Pkt file2 -> Pkt file3 ...

The Sender can interleave blocks within a single file.
The following example shows Encoding Symbols (ES) from different blocks (B) are interleaved. For example:

**(B 1,ES 1)**->(B 2,ES 1)->(B 3,ES 1)->**(B 1,ES 2)**->(B 2,ES 2)...

To configure the multiplexing, use the `Config` struct as follows:

```rust
use flute::sender::Sender;
use flute::sender::Config;
use flute::core::UDPEndpoint;

let config = Config {
    // Transfer a maximum of 3 files in parallel
    multiplex_files: 3,
    // Interleave a maximum of 3 blocks within each file
    interleave_blocks: 3,
    ..Default::default()
};

let endpoint = UDPEndpoint::new(None, "224.0.0.1".to_string(), 3400);
let mut sender = Sender::new(endpoint, 1, &Default::default(), &config);

```

# Python bindings

[![PyPI version](https://badge.fury.io/py/flute-alc.svg)](https://badge.fury.io/py/flute-alc)

## Installation

```bash
pip install flute-alc
```

## Example

Flute Sender python example

```python
    from flute import sender

    # Flute Sender config parameters
    sender_config = sender.Config()

    # Object transmission parameters (no_code => no FEC)
    # encoding symbol size : 1400 bytes
    # Max source block length : 64 encoding symbols
    oti = sender.Oti.new_no_code(1400, 64)

    # Create FLUTE Sender
    flute_sender = sender.Sender(1, oti, sender_config)

    # Transfer a file 
    flute_sender.add_file("/path/to/file", 0, "application/octet-stream", None, None)
    flute_sender.publish()

    while True:
        alc_pkt = flute_sender.read()
        if alc_pkt == None:
            break

        #TODO Send alc_pkt over UDP/IP
```

Flute Receiver python example
```python
    from flute import receiver

    # Write received objects to a destination folder
    receiver_writer = receiver.ObjectWriterBuilder("/path/to/dest")

    # FLUTE Receiver configuration parameters
    receiver_config = receiver.Config()

    tsi = 1

    # Create a FLUTE receiver with the specified endpoint, tsi, writer, and configuration
    udp_endpoint = receiver.UDPEndpoint("224.0.0.1", 1234)
    flute_receiver = receiver.Receiver(udp_endpoint, tsi, receiver_writer, receiver_config)

    while True:
        # Receive LCT/ALC packet from UDP/IP multicast (Implement your own receive_from_udp_socket() function)
        # Note: FLUTE does not handle the UDP/IP layer, you need to implement the socket reception mechanism yourself
        pkt = receive_from_udp_socket()

        # Push the received packet to the FLUTE receiver
        flute_receiver.push(bytes(pkt))
```

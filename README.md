# flute

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
use std::net::UdpSocket;
use std::time::SystemTime;

// Create UDP Socket
let udp_socket = UdpSocket::bind("0.0.0.0:0").unwrap();
udp_socket.connect("224.0.0.1:3400").expect("Connection failed");

// Create FLUTE Sender
let tsi = 1;
let oti = Default::default();
let config = Default::default();
let mut sender = Sender::new(tsi, &oti, &config);

// Add object(s) (files) to the FLUTE sender
let obj = ObjectDesc::create_from_buffer(b"hello world", "text/plain",
&url::Url::parse("file:///hello.txt").unwrap(), 1, None, Cenc::Null, true, None, true).unwrap();
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
use flute::receiver::{objectwriter, MultiReceiver};
use std::net::UdpSocket;
use std::time::SystemTime;

// Create UDP/IP socket to receive FLUTE pkt
let udp_socket = UdpSocket::bind("224.0.0.1:3400").expect("Fail to bind");

// Create a writer able to write received files to the filesystem
let writer = objectwriter::FluteWriterFS::new(&std::path::Path::new("./flute_dir"))
    .unwrap_or_else(|_| std::process::exit(0));

// Create a multi-receiver capable of de-multiplexing several FLUTE sessions
let mut receiver = MultiReceiver::new(None, writer, None);

// Receive pkt from UDP/IP socket and push it to the FLUTE receiver
let mut buf = [0; 2048];
loop {
    let (n, _src) = udp_socket.recv_from(&mut buf).expect("Failed to receive data");
    let now = SystemTime::now();
    receiver.push(&buf[..n], now).unwrap();
    receiver.cleanup(now);
}
```
## Application-Level Forward Erasure Correction (AL-FEC)

The following error recovery algorithms are supported

- [x] No-code
- [x] Reed-Solomon GF 2^8
- [x] Reed-Solomon GF 2^8 Under Specified
- [ ] Reed-Solomon GF 2^16
- [ ] Reed-Solomon GF 2^m
- [ ] RaptorQ

The `Oti` module provides an implementation of the Object Transmission Information (OTI)
used to configure Forward Error Correction (FEC) encoding in the FLUTE protocol.

```rust
use flute::sender::Oti;
use flute::sender::Sender;

// Reed Solomon 2^8 with encoding blocks composed of
// 60 source symbols and 4 repair symbols of 1424 bytes per symbol
let oti = Oti::new_reed_solomon_rs28(1424, 60, 4).unwrap();
let mut sender = Sender::new(1, &oti, &Default::default());
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

let config = Config {
    // Transfer a maximum of 3 files in parallel
    multiplex_files: 3,
    // Interleave a maximum of 3 blocks within each file
    interleave_blocks: 3,
    ..Default::default()
};

let mut sender = Sender::new(1, &Default::default(), &config);

```

License: MIT

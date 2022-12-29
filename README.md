# flute

## FLUTE - File Delivery over Unidirectional Transport

Massively scalable multicast distribution solution

The library implements a unidirectional file delivery, without the need of a return channel.


## RFC

This library implements the following RFCs

| RFC      | Title      | Link       |
| ------------- | ------------- | ------------- |
| RFC 6726 | FLUTE - File Delivery over Unidirectional Transport | <https://www.rfc-editor.org/rfc/rfc6726.html> |
| RFC 5775 | Asynchronous Layered Coding (ALC) Protocol Instantiation | <https://www.rfc-editor.org/rfc/rfc5775.html> |
| RFC 5052 | Forward Error Correction (FEC) Building Block | <https://www.rfc-editor.org/rfc/rfc5052> |
| RFC 5510 | Reed-Solomon Forward Error Correction (FEC) Schemes | <https://www.rfc-editor.org/rfc/rfc5510.html> |

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

- [x] Reed-Solomon GF 2^8
- [ ] Reed-Solomon GF 2^16
- [ ] Reed-Solomon GF 2^m
- [ ] RaptorQ

Object Transmission Information (OTI) configuration to use FEC during transmission

```rust
use flute::sender::Oti;
use flute::sender::FECEncodingID;
use flute::sender::Sender;

let oti = Oti {
    // Select Reed-Solomon GF 2^8
    fec_encoding_id: FECEncodingID::ReedSolomonGF2M,
    // Number of ALC/LCT packet used to repair each block that the object is composed of
    max_number_of_parity_symbols: 3,
    ..Default::default()
};
let mut sender = Sender::new(1, &oti, &Default::default());
```

## Content Encoding (CENC)

The following schemes are supported during the transmission/reception

- [x] Null (no compression)
- [x] Deflate
- [x] Zlib
- [x] Gzip

License: MIT

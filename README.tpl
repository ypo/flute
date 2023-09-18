{{readme}}

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

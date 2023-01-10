{{readme}}

# Python bindings

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
    flute_sender.add_file("/path/to/file",  "application/octet-stream", None, 0, None)
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
    receiver_writer = receiver.FluteWriter("/path/to/dest")

    # FLUTE Receiver configuration parameters
    receiver_config = receiver.Config()

    tsi = 1

    # Creation of a FLUTE receiver
    flute_receiver = receiver.Receiver(tsi, receiver_writer, receiver_config)

    while True:
        # Receive LCT/ALC packet from multicast
        pkt = receive_from_udp_socket()

        # Push packet to the flute receiver
        flute_receiver.push(bytes(pkt))
```

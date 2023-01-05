from unittest import TestCase
import logging


def init():
    FORMAT = '%(levelname)s %(name)s %(asctime)-15s %(filename)s:%(lineno)d %(message)s'
    logging.basicConfig(format=FORMAT)
    logging.getLogger().setLevel(logging.DEBUG)


class SenderTestCase(TestCase):
    from flute import sender

    init()
    print("------- SenderTestCase--------")

    config = sender.Config()
    oti = sender.Oti.new_no_code(1400, 64)
    flute_sender = sender.Sender(1, oti, config)

    buf = bytes(b'hello')
    flute_sender.add_object_from_buffer(buf, "text", "file://hello.txt", None)

    while True:
        pkt = flute_sender.read()
        if pkt == None:
            break

        print("Received pkt of " + str(len(pkt)) + " bytes")

    print("File transmitted !")


class ReceiverTestCase(TestCase):
    from flute import receiver

    init()
    print("------- ReceiverTestCase--------")

    writer = receiver.FluteWriter.new_buffer()
    config = receiver.Config()
    flute_receiver = receiver.Receiver(1, writer, config)

    print("Flute Receiver created !")


class SendReceiveTestCase(TestCase):
    from flute import sender
    from flute import receiver

    init()
    print("------- SendReceiveTestCase--------")

    tsi = 1

    sender_config = sender.Config()
    oti = sender.Oti.new_no_code(1400, 64)
    flute_sender = sender.Sender(tsi, oti, sender_config)

    receiver_writer = receiver.FluteWriter.new_buffer()
    receiver_config = receiver.Config()
    flute_receiver = receiver.Receiver(tsi, receiver_writer, receiver_config)

    while True:
        pkt = flute_sender.read()
        if pkt == None:
            break

        flute_receiver.push(bytes(pkt))

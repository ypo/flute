from unittest import TestCase
import logging


def init():
    FORMAT = '%(levelname)s %(name)s %(asctime)-15s %(filename)s:%(lineno)d %(message)s'
    logging.basicConfig(format=FORMAT)
    logging.getLogger().setLevel(logging.DEBUG)


class SenderReceiverTestCase(TestCase):
    
    
    init()

    def test_create_sender(self):
        from flute import sender
        print("------- test_create_sender--------")
        config = sender.Config()
        oti = sender.Oti.new_no_code(1400, 64)
        flute_sender = sender.Sender(1, oti, config)

        buf = bytes(b'hello')
        flute_sender.add_object_from_buffer(buf, "text", "file://hello.txt", None)
        flute_sender.publish()

        while True:
            pkt = flute_sender.read()
            if pkt == None:
                break

            print("Received pkt of " + str(len(pkt)) + " bytes")

        print("File transmitted !")

    def test_create_receiver(self):
        from flute import receiver
        print("------- test_create_receiver--------")
        writer = receiver.ObjectWriterBuilder.new_buffer()
        config = receiver.Config()
        udp_endpoint = receiver.UDPEndpoint("224.0.0.1", 1234)
        flute_receiver = receiver.Receiver(udp_endpoint, 1, writer, config)
        print("Flute Receiver created !")

    def test_create_multireceiver(self):
        from flute import receiver
        print("------- test_create_multireceiver--------")

        writer = receiver.ObjectWriterBuilder.new_buffer()
        config = receiver.Config()
        
        flute_receiver = receiver.MultiReceiver(writer, config)

        print("Flute Receiver created !")


    def test_send_receiver(self):
        from flute import sender, receiver

        print("------- test_send_receiver--------")

        tsi = 1

        sender_config = sender.Config()
        oti = sender.Oti.new_no_code(1400, 64)
        flute_sender = sender.Sender(tsi, oti, sender_config)

        receiver_writer = receiver.ObjectWriterBuilder.new_buffer()
        receiver_config = receiver.Config()
        udp_endpoint = receiver.UDPEndpoint("224.0.0.1", 1234)
        flute_receiver = receiver.Receiver(udp_endpoint, tsi, receiver_writer, receiver_config)

        buf = bytes(b'hello world')
        flute_sender.add_object_from_buffer(buf, "text", "file://hello.txt", None)
        flute_sender.publish()

        while True:
            pkt = flute_sender.read()
            if pkt == None:
                break

            flute_receiver.push(bytes(pkt))

    def test_send_multi_receiver(self):
        from flute import sender, receiver

        print("------- test_send_multi_receiver--------")

        tsi = 1

        sender_config = sender.Config()
        oti = sender.Oti.new_no_code(1400, 64)
        flute_sender = sender.Sender(tsi, oti, sender_config)

        receiver_writer = receiver.ObjectWriterBuilder.new_buffer()
        receiver_config = receiver.Config()
        flute_receiver = receiver.MultiReceiver(receiver_writer, receiver_config)

        buf = bytes(b'hello world')
        flute_sender.add_object_from_buffer(buf, "text", "file://hello.txt", None)
        flute_sender.publish()

        udp_endpoint = receiver.UDPEndpoint("224.0.0.1", 1234)

        while True:
            pkt = flute_sender.read()
            if pkt == None:
                break

            flute_receiver.push(udp_endpoint, bytes(pkt))

    def test_remove_object(self):
        from flute import sender
        print("------- test_remove_object--------")
        config = sender.Config()
        oti = sender.Oti.new_no_code(1400, 64)
        flute_sender = sender.Sender(1, oti, config)

        buf = bytes(b'hello')
        toi = flute_sender.add_object_from_buffer(buf, "text", "file://hello.txt", None)
        print("object with TOI " + str(toi) + " added")
        assert(flute_sender.nb_objects() == 1)

        success = flute_sender.remove_object(toi)
        assert(success == True)
        assert(flute_sender.nb_objects() == 0)


    def test_lct(self):
        from flute import sender, receiver

        print("------- test_lct--------")

        tsi = 1234

        sender_config = sender.Config()
        oti = sender.Oti.new_no_code(1400, 64)
        flute_sender = sender.Sender(tsi, oti, sender_config)

        receiver_writer = receiver.ObjectWriterBuilder.new_buffer()
        receiver_config = receiver.Config()
        udp_endpoint = receiver.UDPEndpoint("224.0.0.1", 1234)
        flute_receiver = receiver.Receiver(udp_endpoint, tsi, receiver_writer, receiver_config)

        buf = bytes(b'hello world')
        flute_sender.add_object_from_buffer(buf, "text", "file://hello.txt", None)
        flute_sender.publish()

        pkt = flute_sender.read()
        lct = receiver.LCTHeader(bytes(pkt))
        assert(lct.cci == 0)
        assert(lct.tsi == 1234)
        assert(lct.toi == 0)
        assert(lct.sbn == 0)
        assert(lct.esi == 0)

if __name__ == '__main__':
    unittest.main()

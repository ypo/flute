from unittest import TestCase

class SenderTestCase(TestCase):
    from flute import sender
    import logging
    
    FORMAT = '%(levelname)s %(name)s %(asctime)-15s %(filename)s:%(lineno)d %(message)s'
    logging.basicConfig(format=FORMAT)
    logging.getLogger().setLevel(logging.DEBUG)

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

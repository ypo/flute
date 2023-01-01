from unittest import TestCase

class SenderTestCase(TestCase):
    from flute import sender
    import logging

    FORMAT = '%(levelname)s %(name)s %(asctime)-15s %(filename)s:%(lineno)d %(message)s'
    logging.basicConfig(format=FORMAT)
    logging.getLogger().setLevel(logging.DEBUG)

    config = sender.Config()

    config.interleave_blocks = 5

    print(config.interleave_blocks)
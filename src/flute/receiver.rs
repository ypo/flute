use crate::alc;
use crate::alc::objectwriter::ObjectWriter;
use crate::tools::error::Result;
use std::collections::HashMap;
use std::rc::Rc;

pub struct Receiver {
    alc_receiver: HashMap<u64, Box<alc::receiver::Receiver>>,
    tsi: Option<Vec<u64>>,
    writer: Rc<dyn ObjectWriter>,
}

impl Receiver {
    pub fn new(tsi: Option<&Vec<u64>>, writer: Rc<dyn ObjectWriter>) -> Receiver {
        Receiver {
            alc_receiver: HashMap::new(),
            tsi: tsi.map(|f| f.clone()),
            writer,
        }
    }

    pub fn push(&mut self, pkt: &Vec<u8>) -> Result<bool> {
        let alc = alc::alc::parse_alc_pkt(pkt)?;

        let can_handle = match &self.tsi {
            Some(tsi) => tsi.contains(&alc.lct.tsi),
            None => true,
        };

        if !can_handle {
            log::debug!("skip pkt with tsi {}", alc.lct.tsi);
            return Ok(false);
        }

        let receiver = self.get_receiver(alc.lct.tsi);
        receiver.push(&alc)
    }

    fn get_receiver(&mut self, tsi: u64) -> &mut alc::receiver::Receiver {
        self.alc_receiver
            .entry(tsi)
            .or_insert_with(|| Box::new(alc::receiver::Receiver::new(tsi, self.writer.clone())))
            .as_mut()
    }
}

#[cfg(test)]
mod tests {

    use std::{cell::RefCell, rc::Rc, time::SystemTime};

    use crate::alc::objectwriter::ObjectWriterBuffer;
    use crate::alc::{objectdesc, oti, pkt::PktWriter, sender};
    use crate::tools::error::Result;

    struct ReceiverWrapper {
        receiver: RefCell<super::Receiver>,
    }

    impl PktWriter for ReceiverWrapper {
        fn write(&self, data: &Vec<u8>) -> Result<usize> {
            let mut rcv = self.receiver.borrow_mut();
            rcv.push(data)?;
            Ok(data.len())
        }
    }

    fn init() {
        std::env::set_var("RUST_LOG", "debug");
        env_logger::builder().is_test(true).init()
    }

    fn create_sender(writer: Rc<dyn PktWriter>) -> Box<sender::Sender> {
        let oti: oti::Oti = Default::default();
        let sender = Box::new(sender::Sender::new(1, 1, &oti, writer));
        let mut buffer: Vec<u8> = Vec::new();
        buffer.extend(vec![0xAA; oti.encoding_symbol_length as usize * 60 * 2]);
        sender.add_object(
            objectdesc::ObjectDesc::create_from_buffer(
                &buffer,
                "text",
                &url::Url::parse("file:///hello").unwrap(),
                1,
                None,
            )
            .unwrap(),
        );
        sender.publish(&SystemTime::now()).unwrap();
        sender
    }

    #[test]
    pub fn test_receiver() {
        init();

        let writer = ObjectWriterBuffer::new();
        let receiver = Rc::new(ReceiverWrapper {
            receiver: RefCell::new(super::Receiver::new(None, writer)),
        });

        let mut sender = create_sender(receiver);
        loop {
            let ret = sender.run();
            if ret == false {
                break;
            }
        }
    }
}

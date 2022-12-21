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

    use crate::alc::objectwriter::ObjectWriterBuffer;
    use crate::alc::{objectdesc, oti, sender};
    use std::time::SystemTime;

    fn create_sender(buffer: &Vec<u8>, content_location: &url::Url) -> Box<sender::Sender> {
        let oti: oti::Oti = Default::default();
        let sender = Box::new(sender::Sender::new(1, 1, &oti));

        sender.add_object(
            objectdesc::ObjectDesc::create_from_buffer(buffer, "text", content_location, 1, None)
                .unwrap(),
        );
        sender.publish(&SystemTime::now()).unwrap();
        sender
    }

    #[test]
    pub fn test_receiver() {
        crate::tests::init();

        let input_content_location = url::Url::parse("file:///hello").unwrap();
        let mut input_file_buffer: Vec<u8> = Vec::new();
        input_file_buffer.extend(vec![0xAA; 2048]);

        let output = ObjectWriterBuffer::new();
        let mut receiver = super::Receiver::new(None, output.clone());
        let mut sender = create_sender(&input_file_buffer, &input_content_location);

        loop {
            let data = sender.run();
            if data.is_none() {
                break;
            }
            receiver.push(data.as_ref().unwrap()).unwrap();
        }

        let output_session = output.sessions.borrow();
        assert!(output_session.len() == 1);

        let output_object = &output_session[0];
        let output_file_buffer = output_object.data();
        let output_content_location =
            url::Url::parse(output_object.content_location().as_ref().unwrap().as_str()).unwrap();

        assert!(output_file_buffer.eq(&input_file_buffer));
        assert!(output_content_location == output_content_location);
    }
}

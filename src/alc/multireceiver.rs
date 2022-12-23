use super::objectwriter::ObjectWriter;
use super::receiver::{Config, Receiver};
use super::{alc, receiver};
use crate::tools::error::Result;
use std::collections::HashMap;
use std::rc::Rc;

///
/// Multi-sessions FLUTE receiver
/// Demultiplex multiple FLUTE Transport Sessions
/// 
#[derive(Debug)]
pub struct MultiReceiver {
    alc_receiver: HashMap<u64, Box<Receiver>>,
    tsi: Option<Vec<u64>>,
    writer: Rc<dyn ObjectWriter>,
    config: Option<Config>,
}

impl MultiReceiver {
    ///
    /// Creates a new `MultiReceiver` instance, which allows receiving multiple interlaced FLUTE sessions.
    ///
    /// # Arguments
    /// * `tsi` - Optional List of Transport Session Identifier (TSI) accepted by the receiver.
    /// if `None`, all Transport Session are accepted
    ///
    /// * `writer` - Responsible to write object to its final destination.
    ///
    /// * `config` - Configuration of the FLUTE `Receiver`. if `None`, default `Config` will be used
    ///
    /// # Example
    /// ```
    /// // Receive objects from Transport Session 1
    /// use flute::alc::objectwriter::ObjectWriterBuffer;
    /// use flute::alc::multireceiver::MultiReceiver;
    ///
    /// let tsi: u64 = 1;
    /// // Write object to a buffer
    /// let writer = ObjectWriterBuffer::new();
    /// let receiver = MultiReceiver::new(Some(&vec![1]), writer.clone(), None);
    /// ```
    pub fn new(
        tsi: Option<&Vec<u64>>,
        writer: Rc<dyn ObjectWriter>,
        config: Option<receiver::Config>,
    ) -> MultiReceiver {
        MultiReceiver {
            alc_receiver: HashMap::new(),
            tsi: tsi.map(|f| f.clone()),
            writer,
            config,
        }
    }

    ///
    /// Push a ALC/LCT packet to the receiver.
    /// Returns as error the the packet is not a valid ALC/LCT format
    ///
    /// # Arguments
    /// * `pkt`- Payload of the UDP/IP packet.
    ///
    pub fn push(&mut self, pkt: &Vec<u8>) -> Result<()> {
        let alc = alc::parse_alc_pkt(pkt)?;

        let can_handle = match &self.tsi {
            Some(tsi) => tsi.contains(&alc.lct.tsi),
            None => true,
        };

        if !can_handle {
            log::debug!("skip pkt with tsi {}", alc.lct.tsi);
            return Ok(());
        }

        let receiver = self.get_receiver(alc.lct.tsi);
        receiver.push(&alc)
    }

    fn get_receiver(&mut self, tsi: u64) -> &mut Receiver {
        self.alc_receiver
            .entry(tsi)
            .or_insert_with(|| {
                Box::new(Receiver::new(tsi, self.writer.clone(), self.config.clone()))
            })
            .as_mut()
    }
}

#[cfg(test)]
mod tests {

    use crate::alc::objectwriter::ObjectWriterBuffer;
    use crate::alc::{objectdesc, oti, sender};
    use std::time::SystemTime;

    fn create_sender(
        buffer: &Vec<u8>,
        content_location: &url::Url,
        oti: &oti::Oti,
    ) -> Box<sender::Sender> {
        let sender = Box::new(sender::Sender::new(1, 1, &oti));
        sender.add_object(
            objectdesc::ObjectDesc::create_from_buffer(buffer, "text", content_location, 1, None)
                .unwrap(),
        );
        sender.publish(&SystemTime::now()).unwrap();
        sender
    }

    fn run(sender: &mut sender::Sender, receiver: &mut super::MultiReceiver) {
        loop {
            let data = sender.read();
            if data.is_none() {
                break;
            }
            receiver.push(data.as_ref().unwrap()).unwrap();
        }
    }

    fn run_loss(sender: &mut sender::Sender, receiver: &mut super::MultiReceiver) {
        let mut i = 0u32;
        loop {
            let data = sender.read();
            if data.is_none() {
                break;
            }

            if (i & 3) == 0 {
                log::info!("ALC pkt {} is lost", i)
            } else {
                receiver.push(data.as_ref().unwrap()).unwrap();
            }
            i += 1;
        }
    }

    fn check_output(
        input_buffer: &Vec<u8>,
        input_content_location: &url::Url,
        output: &ObjectWriterBuffer,
    ) {
        let output_session = output.sessions.borrow();
        assert!(output_session.len() == 1);

        let output_object = &output_session[0];
        let output_file_buffer = output_object.data();
        let output_content_location =
            url::Url::parse(output_object.content_location().as_ref().unwrap().as_str()).unwrap();

        assert!(output_file_buffer.eq(input_buffer));
        assert!(output_content_location.eq(input_content_location));
    }

    fn create_file_buffer() -> (Vec<u8>, url::Url) {
        let input_content_location = url::Url::parse("file:///hello").unwrap();
        let mut input_file_buffer: Vec<u8> = Vec::new();
        input_file_buffer.extend(vec![0xAA; 2048]);
        (input_file_buffer, input_content_location)
    }

    fn test_receiver_with_oti(oti: &oti::Oti, with_loss: bool) {
        let (input_file_buffer, input_content_location) = create_file_buffer();
        let output = ObjectWriterBuffer::new();
        let mut receiver = super::MultiReceiver::new(None, output.clone(), None);
        let mut sender = create_sender(&input_file_buffer, &input_content_location, &oti);

        if with_loss {
            run_loss(&mut sender, &mut receiver)
        } else {
            run(&mut sender, &mut receiver);
        }
        check_output(&input_file_buffer, &input_content_location, &output);
    }

    #[test]
    pub fn test_receiver_no_code() {
        crate::tests::init();
        test_receiver_with_oti(&Default::default(), false);
    }

    #[test]
    pub fn test_receiver_reed_solomon_gf28_small_block_systematic() {
        crate::tests::init();
        let mut oti: oti::Oti = Default::default();
        oti.fec_encoding_id = oti::FECEncodingID::ReedSolomonGF28SmallBlockSystematic;
        oti.max_number_of_parity_symbols = 3;
        test_receiver_with_oti(&oti, true);
    }

    #[test]
    pub fn test_receiver_reed_solomon_gf28() {
        crate::tests::init();
        let mut oti: oti::Oti = Default::default();
        oti.fec_encoding_id = oti::FECEncodingID::ReedSolomonGF28;
        oti.max_number_of_parity_symbols = 3;
        test_receiver_with_oti(&oti, true);
    }

    #[test]
    pub fn test_receiver_outband_oti() {
        crate::tests::init();
        let mut oti: oti::Oti = Default::default();
        oti.inband_oti = false;
        test_receiver_with_oti(&oti, false);
    }
}

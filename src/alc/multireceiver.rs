use super::objectwriter::FluteWriter;
use super::receiver::{Config, Receiver};
use super::{alc, receiver};
use crate::tools::error::Result;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::SystemTime;

///
/// Multi-sessions FLUTE receiver
/// Demultiplex multiple FLUTE Transport Sessions
///
#[derive(Debug)]
pub struct MultiReceiver {
    alc_receiver: HashMap<u64, Box<Receiver>>,
    tsi: Option<Vec<u64>>,
    writer: Rc<dyn FluteWriter>,
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
    /// use flute::receiver::objectwriter::FluteWriterBuffer;
    /// use flute::receiver::MultiReceiver;
    ///
    /// let tsi: u64 = 1;
    /// // Write object to a buffer
    /// let writer = FluteWriterBuffer::new();
    /// let receiver = MultiReceiver::new(Some(&vec![1]), writer.clone(), None);
    /// ```
    pub fn new(
        tsi: Option<&Vec<u64>>,
        writer: Rc<dyn FluteWriter>,
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
    pub fn push(&mut self, pkt: &[u8], now: std::time::SystemTime) -> Result<()> {
        let alc = alc::parse_alc_pkt(pkt)?;

        let can_handle = match &self.tsi {
            Some(tsi) => tsi.contains(&alc.lct.tsi),
            None => true,
        };

        if !can_handle {
            log::debug!("skip pkt with tsi {}", alc.lct.tsi);
            return Ok(());
        }

        if alc.lct.close_session {
            match self.get_receiver(alc.lct.tsi) {
                Some(receiver) => receiver.push(&alc, now),
                None => {
                    log::warn!(
                        "A session that is not allocated is about to be closed, skip the session"
                    );
                    return Ok(());
                }
            }
        } else {
            let receiver = self.get_receiver_or_create(alc.lct.tsi);
            receiver.push(&alc, now)
        }
    }

    ///
    /// Remove FLUTE session that are closed or expired
    /// Remove Objects that are expired
    ///
    /// Cleanup should be call from time to time to avoid consuming to much memory
    ///
    pub fn cleanup(&mut self, now: SystemTime) {
        self.alc_receiver.retain(|_, v| !v.is_expired());
        for (_, receiver) in &mut self.alc_receiver {
            receiver.cleanup(now);
        }
    }

    fn get_receiver(&mut self, tsi: u64) -> Option<&mut Receiver> {
        self.alc_receiver
            .get_mut(&tsi)
            .map(|receiver| receiver.as_mut())
    }

    fn get_receiver_or_create(&mut self, tsi: u64) -> &mut Receiver {
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

    use rand::RngCore;

    use crate::alc::{lct, objectdesc, oti, sender};
    use crate::receiver::objectwriter::FluteWriterBuffer;

    fn create_sender(
        buffer: &Vec<u8>,
        content_location: &url::Url,
        oti: &oti::Oti,
        cenc: lct::Cenc,
        inband_cenc: bool,
        sender_config: Option<sender::Config>,
    ) -> Box<sender::Sender> {
        let config = sender_config.unwrap_or(sender::Config {
            fdt_cenc: cenc,
            ..Default::default()
        });
        let sender = Box::new(sender::Sender::new(1, &oti, &config));
        sender.add_object(
            objectdesc::ObjectDesc::create_from_buffer(
                buffer,
                "text",
                content_location,
                1,
                None,
                cenc,
                inband_cenc,
                None,
                true,
            )
            .unwrap(),
        );
        sender.publish(std::time::SystemTime::now()).unwrap();
        sender
    }

    fn run(sender: &mut sender::Sender, receiver: &mut super::MultiReceiver) {
        loop {
            let now = std::time::SystemTime::now();
            let data = sender.read(now);
            if data.is_none() {
                break;
            }
            receiver.push(data.as_ref().unwrap(), now).unwrap();
            receiver.cleanup(now);
        }
    }

    fn run_loss(sender: &mut sender::Sender, receiver: &mut super::MultiReceiver) {
        let mut i = 0u32;
        loop {
            let now = std::time::SystemTime::now();
            let data = sender.read(now);
            if data.is_none() {
                break;
            }

            if (i & 3) == 0 {
                log::info!("ALC pkt {} is lost", i)
            } else {
                receiver.push(data.as_ref().unwrap(), now).unwrap();
            }
            receiver.cleanup(now);
            i += 1;
        }
    }

    fn check_output(
        input_buffer: &Vec<u8>,
        input_content_location: &url::Url,
        output: &FluteWriterBuffer,
    ) {
        let output_session = output.objects.borrow();
        assert!(output_session.len() == 1);

        let output_object = &output_session[0];
        let output_file_buffer = output_object.data();
        let output_content_location =
            url::Url::parse(output_object.content_location().as_ref().unwrap().as_str()).unwrap();

        log::info!(
            "Receiver buffer {} expect {}",
            output_file_buffer.len(),
            input_buffer.len()
        );
        assert!(output_object.is_complete() == true);
        assert!(output_object.is_error() == false);
        assert!(output_file_buffer.eq(input_buffer));
        assert!(output_content_location.eq(input_content_location));
    }

    fn create_file_buffer() -> (Vec<u8>, url::Url) {
        let input_content_location = url::Url::parse("file:///hello").unwrap();
        let mut input_file_buffer: Vec<u8> = Vec::new();
        input_file_buffer.extend(vec![0; 4048]);
        
        // Random buffer
        let mut rng = rand::thread_rng();
        rng.fill_bytes(input_file_buffer.as_mut());

        (input_file_buffer, input_content_location)
    }

    fn test_receiver_with_oti(oti: &oti::Oti, with_loss: bool, cenc: lct::Cenc, inband_cenc: bool) {
        let (input_file_buffer, input_content_location) = create_file_buffer();
        let output = FluteWriterBuffer::new();
        let mut receiver = super::MultiReceiver::new(None, output.clone(), None);
        let mut sender = create_sender(
            &input_file_buffer,
            &input_content_location,
            &oti,
            cenc,
            inband_cenc,
            None,
        );

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
        test_receiver_with_oti(&Default::default(), false, lct::Cenc::Null, true);
    }

    #[test]
    pub fn test_receiver_cenc_gzip() {
        crate::tests::init();
        test_receiver_with_oti(&Default::default(), false, lct::Cenc::Gzip, true);
    }

    #[test]
    pub fn test_receiver_cenc_deflate() {
        crate::tests::init();
        test_receiver_with_oti(&Default::default(), false, lct::Cenc::Deflate, true);
    }

    #[test]
    pub fn test_receiver_cenc_zlib() {
        crate::tests::init();
        test_receiver_with_oti(&Default::default(), false, lct::Cenc::Zlib, true);
    }

    #[test]
    pub fn test_receiver_reed_solomon_gf28_small_block_systematic() {
        crate::tests::init();
        let mut oti: oti::Oti = Default::default();
        oti.fec_encoding_id = oti::FECEncodingID::ReedSolomonGF28SmallBlockSystematic;
        oti.max_number_of_parity_symbols = 3;
        test_receiver_with_oti(&oti, true, lct::Cenc::Null, true);
    }

    #[test]
    pub fn test_receiver_reed_solomon_gf28() {
        crate::tests::init();
        let mut oti: oti::Oti = Default::default();
        oti.fec_encoding_id = oti::FECEncodingID::ReedSolomonGF28;
        oti.max_number_of_parity_symbols = 3;
        test_receiver_with_oti(&oti, true, lct::Cenc::Null, true);
    }

    #[test]
    pub fn test_receiver_outband_oti() {
        crate::tests::init();
        let mut oti: oti::Oti = Default::default();
        oti.inband_oti = false;
        test_receiver_with_oti(&oti, false, lct::Cenc::Null, true);
    }

    #[test]
    pub fn test_receiver_outband_cenc() {
        crate::tests::init();
        let oti: oti::Oti = Default::default();
        test_receiver_with_oti(&oti, false, lct::Cenc::Null, false);
    }

    #[test]
    pub fn test_receiver_outband_cenc_and_oti() {
        crate::tests::init();
        let mut oti: oti::Oti = Default::default();
        oti.inband_oti = false;
        test_receiver_with_oti(&oti, false, lct::Cenc::Null, false);
    }

    #[test]
    pub fn test_receiver_expired_fdt() {
        crate::tests::init();

        let oti: oti::Oti = Default::default();
        let (input_file_buffer, input_content_location) = create_file_buffer();
        let output = FluteWriterBuffer::new();
        let mut receiver = super::MultiReceiver::new(None, output.clone(), None);
        let mut sender = create_sender(
            &input_file_buffer,
            &input_content_location,
            &oti,
            lct::Cenc::Null,
            true,
            Some(sender::Config {
                fdt_duration: std::time::Duration::from_secs(1),
                fdt_inband_sct: false,
                ..Default::default()
            }),
        );

        loop {
            let now_sender = std::time::SystemTime::now();
            let data = sender.read(now_sender);
            if data.is_none() {
                break;
            }

            // Simulate reception 60s later -> FDT should be expired
            let now_receiver = std::time::SystemTime::now() + std::time::Duration::from_secs(60);
            receiver.push(data.as_ref().unwrap(), now_receiver).unwrap();
            receiver.cleanup(now_receiver);
        }

        let nb_complete_objects = output
            .objects
            .borrow()
            .iter()
            .filter(|&obj| obj.is_complete())
            .count();

        let nb_error_objects = output
            .objects
            .borrow()
            .iter()
            .filter(|&obj| obj.is_error())
            .count();

        assert!(nb_complete_objects == 0);
        assert!(nb_error_objects == 0);
    }
}

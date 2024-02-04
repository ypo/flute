mod tests {
    use flute::core::UDPEndpoint;
    use flute::sender::PriorityQueue;
    use rand::RngCore;
    use std::io::Write;
    use std::rc::Rc;

    use flute::receiver;
    use flute::sender;

    pub fn init() {
        // std::env::set_var("RUST_LOG", "debug");
        env_logger::builder().is_test(true).try_init().ok();
    }

    fn create_sender(
        objects: Vec<Box<sender::ObjectDesc>>,
        oti: &sender::Oti,
        fdt_cenc: sender::Cenc,
        sender_config: Option<sender::Config>,
    ) -> Box<sender::Sender> {
        let config = sender_config.unwrap_or(sender::Config {
            fdt_cenc: fdt_cenc,
            ..Default::default()
        });
        let endpoint = UDPEndpoint::new(None, "224.0.0.1".to_owned(), 5000);
        let mut sender = Box::new(sender::Sender::new(endpoint, 1, &oti, &config));

        for obj in objects {
            sender.add_object(0, obj).unwrap();
        }
        sender.publish(std::time::SystemTime::now()).unwrap();
        sender
    }

    fn create_object(
        transfer_file_size: usize,
        content_type: &str,
        cenc: sender::Cenc,
        inband_cenc: bool,
        object_oti: Option<&sender::Oti>,
    ) -> (Box<sender::ObjectDesc>, Vec<u8>) {
        let (buffer, content_location) = create_file_buffer(transfer_file_size);
        (
            sender::ObjectDesc::create_from_buffer(
                &buffer,
                &content_type,
                &content_location,
                1,
                None,
                None,
                None,
                cenc,
                inband_cenc,
                object_oti.map(|e| e.clone()),
                true,
            )
            .unwrap(),
            buffer,
        )
    }

    fn create_temp_file_object(
        transfer_file_size: usize,
        content_type: &str,
        cenc: sender::Cenc,
        inband_cenc: bool,
        object_oti: Option<&sender::Oti>,
    ) -> (Box<sender::ObjectDesc>, Vec<u8>) {
        let (buffer, content_location) = create_file_buffer(transfer_file_size);
        let file_path = std::env::temp_dir().join("flute_object_test.bin");
        let mut file = std::fs::File::create(&file_path).unwrap();
        file.write_all(&buffer).unwrap();
        (
            sender::ObjectDesc::create_from_file(
                &file_path,
                Some(&content_location),
                &content_type,
                false,
                1,
                None,
                None,
                None,
                cenc,
                inband_cenc,
                object_oti.map(|e| e.clone()),
                true,
            )
            .unwrap(),
            buffer,
        )
    }

    fn delete_temp_file() {
        let file_path = std::env::temp_dir().join("flute_object_test.bin");
        std::fs::remove_file(file_path).ok();
    }

    fn run(sender: &mut sender::Sender, receiver: &mut receiver::MultiReceiver) {
        let endpoint = UDPEndpoint::new(None, "224.0.0.1".to_owned(), 5000);
        loop {
            let now = std::time::SystemTime::now();
            let data = sender.read(now);
            if data.is_none() {
                break;
            }
            receiver
                .push(&endpoint, data.as_ref().unwrap(), now)
                .unwrap();
            receiver.cleanup(now);
        }
    }

    fn run_loss(sender: &mut sender::Sender, receiver: &mut receiver::MultiReceiver) {
        let mut i = 0u32;
        let endpoint = UDPEndpoint::new(None, "224.0.0.1".to_owned(), 5000);
        loop {
            let now = std::time::SystemTime::now();
            let data = sender.read(now);
            if data.is_none() {
                break;
            }

            if (i & 7) == 0 {
                log::info!("ALC pkt {} is lost", i)
            } else {
                receiver
                    .push(&endpoint, data.as_ref().unwrap(), now)
                    .unwrap();
            }
            receiver.cleanup(now);
            i += 1;
        }
    }

    fn check_output(
        input_buffer: &[u8],
        input_content_location: &url::Url,
        input_content_type: &str,
        output: &receiver::writer::ObjectWriterBufferBuilder,
    ) {
        let output_session = output.objects.borrow();
        assert!(output_session.len() == 1);

        let output_object = output_session[0].as_ref().borrow();
        let output_file_buffer: &[u8] = output_object.data.as_ref();
        let output_meta = output_object.meta.as_ref().unwrap();

        log::info!(
            "Receiver buffer {} expect {}",
            output_file_buffer.len(),
            input_buffer.len()
        );
        assert!(output_object.complete == true);
        assert!(output_object.error == false);
        assert!(output_file_buffer.eq(input_buffer));
        assert!(output_meta.content_location.eq(input_content_location));
        assert!(output_meta.content_length.unwrap() == input_buffer.len());
        assert!(output_meta
            .content_type
            .as_ref()
            .unwrap()
            .eq(input_content_type));
    }

    fn create_file_buffer(file_size: usize) -> (Vec<u8>, url::Url) {
        let input_content_location = url::Url::parse("file:///hello").unwrap();
        let mut input_file_buffer: Vec<u8> = Vec::new();
        input_file_buffer.extend(vec![0; file_size]);

        // Random buffer
        let mut rng = rand::thread_rng();
        rng.fill_bytes(input_file_buffer.as_mut());

        (input_file_buffer, input_content_location)
    }

    fn test_receiver_with_oti(
        oti: &sender::Oti,
        object_oti: Option<&sender::Oti>,
        with_loss: bool,
        cenc: sender::Cenc,
        inband_cenc: bool,
        sender_config: Option<sender::Config>,
        transfer_file_size: usize,
        temp_file: bool,
    ) {
        let content_type = "application/octet-stream";

        let (obj, input_file_buffer) = match temp_file {
            true => create_temp_file_object(
                transfer_file_size,
                content_type,
                cenc,
                inband_cenc,
                object_oti,
            ),
            _ => create_object(
                transfer_file_size,
                content_type,
                cenc,
                inband_cenc,
                object_oti,
            ),
        };

        let input_content_location = obj.content_location.clone();

        let output = Rc::new(receiver::writer::ObjectWriterBufferBuilder::new());
        let mut receiver = receiver::MultiReceiver::new(output.clone(), None, false);
        let mut sender = create_sender(vec![obj], &oti, cenc, sender_config);

        if with_loss {
            run_loss(&mut sender, &mut receiver)
        } else {
            run(&mut sender, &mut receiver);
        }

        if temp_file {
            delete_temp_file();
        }

        check_output(
            &input_file_buffer,
            &input_content_location,
            content_type,
            &output,
        );
    }

    #[test]
    pub fn test_receiver_no_code() {
        init();
        test_receiver_with_oti(
            &sender::Oti::new_no_code(1400, 64),
            None,
            false,
            sender::Cenc::Null,
            true,
            None,
            100000,
            false,
        );
    }

    #[test]
    pub fn test_receiver_no_code_temp_file() {
        init();
        test_receiver_with_oti(
            &sender::Oti::new_no_code(1400, 64),
            None,
            false,
            sender::Cenc::Null,
            true,
            None,
            100000,
            true,
        );
    }

    #[test]
    pub fn test_receiver_no_code_large_temp_file() {
        init();
        test_receiver_with_oti(
            &sender::Oti::new_no_code(1400, 64),
            None,
            false,
            sender::Cenc::Null,
            true,
            None,
            100000000,
            true,
        );
    }

    #[test]
    pub fn test_receiver_no_code_no_multiplex() {
        crate::tests::init();
        test_receiver_with_oti(
            &Default::default(),
            None,
            false,
            sender::Cenc::Null,
            true,
            Some(sender::Config {
                interleave_blocks: 1,
                priority_queues: std::collections::BTreeMap::from([(
                    0,
                    sender::PriorityQueue { multiplex_files: 0 },
                )]),
                ..Default::default()
            }),
            100000,
            false,
        );
    }

    #[test]
    pub fn test_receiver_cenc_gzip() {
        crate::tests::init();
        test_receiver_with_oti(
            &Default::default(),
            None,
            false,
            sender::Cenc::Gzip,
            true,
            None,
            100000,
            false,
        );
    }

    #[test]
    pub fn test_receiver_cenc_deflate() {
        crate::tests::init();
        test_receiver_with_oti(
            &Default::default(),
            None,
            false,
            sender::Cenc::Deflate,
            true,
            None,
            100000,
            false,
        );
    }

    #[test]
    pub fn test_receiver_cenc_zlib() {
        crate::tests::init();
        test_receiver_with_oti(
            &Default::default(),
            None,
            false,
            sender::Cenc::Zlib,
            true,
            None,
            100000,
            false,
        );
    }

    #[test]
    pub fn test_receiver_reed_solomon_gf28_under_specified() {
        crate::tests::init();
        let oti: sender::Oti =
            sender::Oti::new_reed_solomon_rs28_under_specified(1400, 64, 20).unwrap();
        test_receiver_with_oti(
            &oti,
            None,
            true,
            sender::Cenc::Null,
            true,
            None,
            100000,
            false,
        );
    }

    #[test]
    pub fn test_receiver_reed_solomon_gf28() {
        crate::tests::init();
        let oti: sender::Oti = sender::Oti::new_reed_solomon_rs28(1400, 64, 20).unwrap();
        test_receiver_with_oti(
            &oti,
            None,
            true,
            sender::Cenc::Null,
            true,
            None,
            100000,
            false,
        );
    }

    #[test]
    pub fn test_receiver_fdt_raptorq_object_reed_solomon_gf28() {
        crate::tests::init();
        let oti: sender::Oti = sender::Oti::new_raptorq(1400, 64, 20, 1, 4).unwrap();
        let oti_object: sender::Oti = sender::Oti::new_reed_solomon_rs28(1400, 64, 20).unwrap();
        test_receiver_with_oti(
            &oti,
            Some(&oti_object),
            true,
            sender::Cenc::Null,
            true,
            None,
            100000,
            false,
        );
    }

    #[test]
    pub fn test_receiver_reed_solomon_gf28_outband_fti() {
        crate::tests::init();
        let mut oti: sender::Oti = sender::Oti::new_reed_solomon_rs28(1400, 64, 20).unwrap();
        oti.inband_fti = false;
        test_receiver_with_oti(
            &oti,
            None,
            true,
            sender::Cenc::Null,
            true,
            None,
            100000,
            false,
        );
    }

    #[test]
    pub fn test_receiver_raptorq() {
        crate::tests::init();
        let oti: sender::Oti = sender::Oti::new_raptorq(1400, 64, 20, 1, 4).unwrap();
        test_receiver_with_oti(
            &oti,
            None,
            true,
            sender::Cenc::Null,
            true,
            None,
            100000,
            false,
        );
    }

    #[test]
    pub fn test_receiver_raptor() {
        crate::tests::init();
        let oti: sender::Oti = sender::Oti::new_raptor(1400, 64, 20, 1, 4).unwrap();
        test_receiver_with_oti(
            &oti,
            None,
            true,
            sender::Cenc::Null,
            true,
            None,
            100000,
            false,
        );
    }

    #[test]
    pub fn test_receiver_raptorq_outband_fti() {
        crate::tests::init();
        let mut oti: sender::Oti = sender::Oti::new_raptorq(1400, 64, 20, 1, 4).unwrap();
        oti.inband_fti = false;
        test_receiver_with_oti(
            &oti,
            None,
            true,
            sender::Cenc::Null,
            true,
            None,
            100000,
            false,
        );
    }

    #[test]
    pub fn test_receiver_outband_fti() {
        crate::tests::init();
        let mut oti: sender::Oti = Default::default();
        oti.inband_fti = false;
        test_receiver_with_oti(
            &oti,
            None,
            false,
            sender::Cenc::Null,
            true,
            None,
            100000,
            false,
        );
    }

    #[test]
    pub fn test_receiver_outband_cenc() {
        crate::tests::init();
        let oti: sender::Oti = Default::default();
        test_receiver_with_oti(
            &oti,
            None,
            false,
            sender::Cenc::Null,
            false,
            None,
            100000,
            false,
        );
    }

    #[test]
    pub fn test_receiver_outband_cenc_and_fti() {
        crate::tests::init();
        let mut oti: sender::Oti = Default::default();
        oti.inband_fti = false;
        test_receiver_with_oti(
            &oti,
            None,
            false,
            sender::Cenc::Null,
            false,
            None,
            100000,
            false,
        );
    }

    #[test]
    pub fn test_receiver_expired_fdt() {
        crate::tests::init();

        let oti: sender::Oti = Default::default();
        let content_type = "application/octet-stream";
        let (obj, _) = create_object(100000, content_type, sender::Cenc::Null, true, None);
        let output = Rc::new(receiver::writer::ObjectWriterBufferBuilder::new());
        let mut receiver = receiver::MultiReceiver::new(output.clone(), None, false);
        let mut sender = create_sender(
            vec![obj],
            &oti,
            sender::Cenc::Null,
            Some(sender::Config {
                fdt_duration: std::time::Duration::from_secs(30),
                fdt_inband_sct: false,
                ..Default::default()
            }),
        );

        let endpoint = UDPEndpoint::new(None, "224.0.0.1".to_owned(), 5000);

        loop {
            let now_sender = std::time::SystemTime::now();
            let data = sender.read(now_sender);
            if data.is_none() {
                break;
            }

            // Simulate reception 60s later -> FDT should be expired
            let now_receiver = std::time::SystemTime::now() + std::time::Duration::from_secs(60);
            receiver
                .push(&endpoint, data.as_ref().unwrap(), now_receiver)
                .unwrap();
            receiver.cleanup(now_receiver);
        }

        let nb_complete_objects = output
            .as_ref()
            .objects
            .borrow()
            .iter()
            .filter(|&obj| obj.borrow().complete)
            .count();

        let nb_error_objects = output
            .as_ref()
            .objects
            .borrow()
            .iter()
            .filter(|&obj| obj.borrow().error)
            .count();

        assert!(nb_complete_objects == 0);
        assert!(nb_error_objects == 0);
    }

    #[test]
    pub fn test_receiver_empty_file() {
        init();
        test_receiver_with_oti(
            &sender::Oti::new_no_code(1400, 64),
            None,
            false,
            sender::Cenc::Null,
            true,
            None,
            0,
            false,
        );
    }

    #[test]
    fn test_priority_queues() {
        let content_type = "application/octet-stream";

        let oti = sender::Oti::new_no_code(1400, 64);

        let (high_priority_obj, high_priority_buffer) =
            create_object(1024, content_type, sender::Cenc::Null, true, Some(&oti));

        let (low_priority_obj, low_priority_buffer) =
            create_object(1024, content_type, sender::Cenc::Null, true, Some(&oti));

        let output = Rc::new(receiver::writer::ObjectWriterBufferBuilder::new());
        let mut receiver = receiver::MultiReceiver::new(output.clone(), None, false);

        let mut sender_config: sender::Config = Default::default();
        sender_config.set_priority_queue(PriorityQueue::HIGHEST, PriorityQueue::new(3));
        sender_config.set_priority_queue(PriorityQueue::LOW, PriorityQueue::new(3));

        let endpoint = UDPEndpoint::new(None, "224.0.0.1".to_owned(), 5000);
        let mut sender = Box::new(sender::Sender::new(endpoint, 1, &oti, &sender_config));

        sender
            .add_object(PriorityQueue::LOW, low_priority_obj)
            .unwrap();
        sender
            .add_object(PriorityQueue::HIGHEST, high_priority_obj)
            .unwrap();
        sender.publish(std::time::SystemTime::now()).unwrap();

        run(&mut sender, &mut receiver);

        let output_session = output.objects.borrow();
        assert!(output_session.len() == 2);

        // Verify that file transferred in high priority queue is received before file in low priority queue

        let high_priority_output_object = output_session[0].as_ref().borrow();
        let high_priority_output_file_buffer: &[u8] = high_priority_output_object.data.as_ref();

        let low_priority_output_object = output_session[1].as_ref().borrow();
        let low_priority_output_file_buffer: &[u8] = low_priority_output_object.data.as_ref();

        assert!(high_priority_output_object.complete == true);
        assert!(high_priority_output_object.error == false);
        assert!(high_priority_output_file_buffer.eq(&high_priority_buffer));

        assert!(low_priority_output_object.complete == true);
        assert!(low_priority_output_object.error == false);
        assert!(low_priority_output_file_buffer.eq(&low_priority_buffer));
    }
}

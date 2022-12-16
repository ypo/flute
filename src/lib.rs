mod fec;

pub mod alc;
pub mod network;
pub mod tools;

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    fn init() {
        std::env::set_var("RUST_LOG", "debug");
        env_logger::builder().is_test(true).init()
    }

    #[test]
    pub fn test_flute_sender() {
        init();

        let oti: super::alc::oti::Oti = Default::default();
        let writer = Rc::new(super::network::udpwriter::UdpWriter::new("224.0.0.1:3004").unwrap());
        let mut sender = super::alc::sender::Sender::new(1, 1, &oti, writer);

        let obj = super::alc::objectdesc::ObjectDesc::create_from_buffer(
            &vec![1, 2, 3],
            "binary",
            &url::Url::parse("file:///hello.bin").unwrap(),
            1,
            None,
        )
        .unwrap();

        sender.add_object(obj);
        sender.publish();
        loop {
            let ret = sender.run();
            if ret == false {
                break;
            }
        }
    }
}

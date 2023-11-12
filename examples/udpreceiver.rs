use flute::{
    core::UDPEndpoint,
    receiver::{writer, MultiReceiver},
};
use std::{net::UdpSocket, rc::Rc};

fn main() {
    std::env::set_var("RUST_LOG", "info");
    env_logger::builder().try_init().ok();

    let endpoint = UDPEndpoint::new(None, "224.0.0.1".to_string(), 3400);

    let args: Vec<String> = std::env::args().collect();
    if args.len() == 1 {
        println!(
            "Save FLUTE objects to a destination folder received from UDP/FLUTE {:?}",
            endpoint
        );
        println!("Usage: {} path/to/destination_folder", args[0]);
        std::process::exit(0);
    }

    let dest_dir = std::path::Path::new(&args[1]);
    if !dest_dir.is_dir() {
        log::error!("{:?} is not a directory", dest_dir);
        std::process::exit(-1);
    }

    log::info!("Create UDP Socket");
    let udp_socket = UdpSocket::bind(format!(
        "{}/{}",
        endpoint.destination_group_address, endpoint.port
    ))
    .expect("Fail to bind");

    log::info!("Create FLUTE, write objects to {:?}", dest_dir);
    let writer = Rc::new(writer::ObjectWriterFSBuilder::new(dest_dir).unwrap());
    let mut receiver = MultiReceiver::new(writer, None, false);

    let mut buf = [0; 2048];
    loop {
        let (n, _src) = udp_socket
            .recv_from(&mut buf)
            .expect("Failed to receive data");

        let now = std::time::SystemTime::now();
        match receiver.push(&endpoint, &buf[..n], now) {
            Err(_) => log::error!("Wrong ALC/LCT packet"),
            _ => {}
        };
        receiver.cleanup(now);
    }
}

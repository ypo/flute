use flute::receiver::{objectwriter, MultiReceiver};
use std::net::UdpSocket;

fn main() {
    std::env::set_var("RUST_LOG", "info");
    env_logger::builder().try_init().ok();
    let multicast_addr = "224.0.0.1:3400";

    let args: Vec<String> = std::env::args().collect();
    if args.len() == 1 {
        println!(
            "Save FLUTE objects to a destination folder received from UDP/FLUTE {}",
            multicast_addr
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
    let udp_socket = UdpSocket::bind(multicast_addr).expect("Fail to bind");

    log::info!("Create FLUTE, write objects to {:?}", dest_dir);
    let writer = objectwriter::FluteWriterFS::new(dest_dir).unwrap();
    let mut receiver = MultiReceiver::new(None, writer, None);

    let mut buf = [0; 2048];
    loop {
        let (n, _src) = udp_socket
            .recv_from(&mut buf)
            .expect("Failed to receive data");

        match receiver.push(&buf[..n]) {
            Err(_) => log::error!("Wrong ALC/LCT packet"),
            _ => {}
        };
    }
}

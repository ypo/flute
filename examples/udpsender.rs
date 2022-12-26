use flute::sender::{ObjectDesc, Sender, CENC};
use std::{net::UdpSocket, time::SystemTime};

fn main() {
    std::env::set_var("RUST_LOG", "info");
    env_logger::builder().is_test(true).try_init().ok();

    let args: Vec<String> = std::env::args().collect();
    if args.len() == 1 {
        println!("{} path/to/file1 path/to/file2 ...", args[0]);
        std::process::exit(0);
    }

    let tsi = 1;
    let fdtid = 1;

    log::info!("Create UDP Socket");
    let udp_socket = UdpSocket::bind("0.0.0.0:0").unwrap();

    log::info!("Create FLUTE Sender");
    let mut sender = Sender::new(tsi, fdtid, &Default::default(), CENC::Null);

    let dest = "224.0.0.1:3400";
    log::info!("Connect to {}", dest);
    udp_socket.connect(dest).expect("Connection failed");

    for file in &args[1..] {
        let path = std::path::Path::new(file);

        if !path.is_file() {
            log::error!("{} is not a file", file);
            std::process::exit(-1);
        }

        log::info!("Insert file {} to FLUTE sender", file);
        let obj = Box::new(ObjectDesc::create_from_file(
            path,
            None,
            "application/octet-stream",
            1,
            None,
            CENC::Null,
            true,
            None,
            true,
        ))
        .unwrap();
        sender.add_object(obj);
    }

    log::info!("Publish FDT update");
    sender.publish(&SystemTime::now()).unwrap();

    loop {
        let pkt = sender.read();
        if pkt.is_none() {
            break;
        }
        udp_socket.send(&pkt.unwrap()).unwrap();
    }
}

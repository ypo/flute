use clap::Parser;
use flute::{
    core::UDPEndpoint,
    sender::{ObjectDesc, Sender},
};
use std::{net::UdpSocket, time::SystemTime};

#[derive(Parser)]
#[command(name = "flute-sender", about = "Send files over UDP/FLUTE")]
struct Cli {
    /// Destination address (multicast group)
    #[arg(short, long, default_value = "224.0.0.1")]
    destination: String,

    /// Destination port
    #[arg(short, long, default_value_t = 3400)]
    port: u16,

    /// Files to send
    #[arg(required = true)]
    files: Vec<String>,
}

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .try_init()
        .ok();

    let cli = Cli::parse();
    let dest = format!("{}:{}", cli.destination, cli.port);
    let endpoint = UDPEndpoint::new(None, cli.destination.clone(), cli.port);

    log::info!("Create UDP Socket");
    let udp_socket = UdpSocket::bind("0.0.0.0:0").unwrap();

    log::info!("Create FLUTE Sender");
    let tsi = 1;
    let mut sender = Sender::new(endpoint, tsi, &Default::default(), &Default::default());

    log::info!("Connect to {}", dest);
    udp_socket.connect(&dest).expect("Connection failed");

    for file in &cli.files {
        let path = std::path::Path::new(file);

        if !path.is_file() {
            log::error!("{} is not a file", file);
            std::process::exit(1);
        }

        log::info!("Insert file {} to FLUTE sender", file);
        let obj = ObjectDesc::create_from_file(
            path,
            None,
            "application/octet-stream",
            true,
            true,
            Default::default(),
        )
        .unwrap();
        sender.add_object(0, obj).expect("Add object failed");
    }

    log::info!("Publish FDT update");
    sender.publish(SystemTime::now()).expect("Publish failed");

    // Send a "close session" packet to notify the receiver that the
    // previous session should be terminated, a new one is about to start.
    let close_session_pkt = sender.read_close_session(SystemTime::now());
    udp_socket.send(&close_session_pkt).expect("Send failed");

    while let Some(pkt) = sender.read(SystemTime::now()) {
        udp_socket.send(&pkt).expect("Send failed");
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

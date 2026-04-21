use clap::Parser;
use flute::{
    core::UDPEndpoint,
    receiver::{writer, MultiReceiver},
};
use std::rc::Rc;

mod msocket;

#[derive(Parser)]
#[command(name = "flute-receiver", about = "Receive files from UDP/FLUTE")]
struct Cli {
    /// Multicast group address
    #[arg(short, long, default_value = "224.0.0.1")]
    group: String,

    /// Port
    #[arg(short, long, default_value_t = 3400)]
    port: u16,

    /// Network interface IP address to bind to
    #[arg(short, long, default_value = "127.0.0.1")]
    interface: String,

    /// Destination directory for received files
    #[arg(required = true)]
    dest_dir: String,
}

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .try_init()
        .ok();

    let cli = Cli::parse();

    let endpoint = UDPEndpoint::new(None, cli.group.clone(), cli.port);

    let dest_dir = std::path::Path::new(&cli.dest_dir);
    if !dest_dir.is_dir() {
        log::error!("{:?} is not a directory", dest_dir);
        std::process::exit(1);
    }

    log::info!("Create FLUTE receiver, write objects to {:?}", dest_dir);

    let mut config = flute::receiver::Config::default();
    config.object_receive_once = true;

    let enable_md5_check = true;
    let writer = Rc::new(writer::ObjectWriterFSBuilder::new(dest_dir, enable_md5_check).unwrap());
    let mut receiver = MultiReceiver::new(writer, Some(config), false);

    let socket = msocket::MSocket::new(&endpoint, Some(&cli.interface), false)
        .expect("Fail to create Multicast Socket");

    let mut buf = [0; 2048];
    loop {
        let (n, _src) = socket
            .sock
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

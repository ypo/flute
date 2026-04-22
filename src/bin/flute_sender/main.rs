use clap::Parser;
use flute::{
    core::{lct::Cenc, Oti, UDPEndpoint},
    sender::{CarouselRepeatMode, Config, CreateFromFile, PriorityQueue, Sender, TransferConfig},
};
use std::{net::UdpSocket, time::SystemTime};

mod token_bucket;

#[derive(Parser)]
#[command(name = "flute-sender", about = "Send files over UDP/FLUTE")]
struct Cli {
    /// Destination address (multicast group)
    #[arg(short, long, default_value = "224.0.0.1")]
    destination: String,

    /// Destination port
    #[arg(short, long, default_value_t = 3400)]
    port: u16,

    /// Local address to bind the UDP socket to (e.g. 192.168.1.10:0)
    #[arg(long, default_value = "0.0.0.0:0")]
    bind: String,

    /// Content encoding: null, zlib, deflate, gzip
    #[arg(long, default_value = "null")]
    cenc: String,

    /// TSI (Transport Session Identifier)
    #[arg(long, default_value_t = 1)]
    tsi: u64,

    /// Max number of times each file is transferred
    #[arg(long, default_value_t = 1)]
    max_transfer_count: u32,

    /// Disable MD5 computation
    #[arg(long)]
    no_md5: bool,

    /// Do not cache files in RAM (stream from disk)
    #[arg(long)]
    no_cache_in_ram: bool,

    /// Carousel mode: "delay" (fixed delay after each transfer) or "interval" (fixed interval between transfer starts)
    #[arg(long)]
    carousel: Option<String>,

    /// Carousel duration in seconds (requires --carousel)
    #[arg(long, default_value_t = 1)]
    carousel_secs: u64,

    /// FEC scheme: no-code, reed-solomon, reed-solomon-us, raptorq, raptor
    #[arg(long, default_value = "no-code")]
    fec: String,

    /// Encoding symbol length in bytes
    #[arg(long, default_value_t = 1400)]
    symbol_length: u16,

    /// Maximum source block length (number of source symbols per block)
    #[arg(long, default_value_t = 64)]
    source_block_length: u16,

    /// Maximum number of parity (repair) symbols (for reed-solomon, raptorq, raptor)
    #[arg(long, default_value_t = 4)]
    parity_symbols: u16,

    /// Max number of blocks interleaved during transmission of a file
    #[arg(long, default_value_t = 1)]
    interleave_blocks: u8,

    /// Max number of files multiplexed in parallel
    #[arg(long, default_value_t = 3)]
    multiplex_files: u32,

    /// Transmission bitrate in kbit/s (0 = unlimited)
    #[arg(short, long, default_value_t = 30000)]
    bitrate: u64,

    /// Files to send
    #[arg(required = true)]
    files: Vec<String>,
}

fn main() {
    env_logger::init();

    let cli = Cli::parse();
    let dest = format!("{}:{}", cli.destination, cli.port);
    let endpoint = UDPEndpoint::new(None, cli.destination.clone(), cli.port);

    let cenc = match cli.cenc.as_str() {
        "null" => Cenc::Null,
        "zlib" => Cenc::Zlib,
        "deflate" => Cenc::Deflate,
        "gzip" => Cenc::Gzip,
        other => {
            log::error!(
                "Unknown cenc '{}', expected: null, zlib, deflate, gzip",
                other
            );
            std::process::exit(1);
        }
    };

    let oti = match cli.fec.as_str() {
        "no-code" => Oti::new_no_code(cli.symbol_length, cli.source_block_length),
        "reed-solomon" => Oti::new_reed_solomon_rs28(
            cli.symbol_length,
            cli.source_block_length as u8,
            cli.parity_symbols as u8,
        )
        .expect("Invalid Reed-Solomon RS28 parameters"),
        "raptorq" => Oti::new_raptorq(
            cli.symbol_length,
            cli.source_block_length,
            cli.parity_symbols,
            1,
            4,
        )
        .expect("Invalid RaptorQ parameters"),
        "raptor" => Oti::new_raptor(
            cli.symbol_length,
            cli.source_block_length,
            cli.parity_symbols,
            1,
            4,
        )
        .expect("Invalid Raptor parameters"),
        other => {
            log::error!(
                "Unknown FEC '{}', expected: no-code, reed-solomon, reed-solomon-us, raptorq, raptor",
                other
            );
            std::process::exit(1);
        }
    };

    log::info!("Create UDP Socket");
    let udp_socket = UdpSocket::bind(&cli.bind).expect("Failed to bind socket");

    log::info!("Create FLUTE Sender");
    let mut sender_config = Config {
        interleave_blocks: cli.interleave_blocks,
        ..Default::default()
    };
    sender_config.set_priority_queue(
        PriorityQueue::HIGHEST,
        PriorityQueue::new(cli.multiplex_files),
    );
    let mut sender = Sender::new(endpoint, cli.tsi, &oti, &sender_config);

    log::info!("Connect to {}", dest);
    udp_socket.connect(&dest).expect("Connection failed");

    let compute_md5 = !cli.no_md5;
    let cache_in_ram = !cli.no_cache_in_ram;
    let carousel_duration = std::time::Duration::from_secs(cli.carousel_secs);
    let carousel_mode = cli.carousel.as_deref().map(|mode| match mode {
        "delay" => CarouselRepeatMode::DelayBetweenTransfers(carousel_duration),
        "interval" => CarouselRepeatMode::IntervalBetweenStartTimes(carousel_duration),
        other => {
            log::error!(
                "Unknown carousel mode '{}', expected: delay, interval",
                other
            );
            std::process::exit(1);
        }
    });

    for file in &cli.files {
        let path = std::path::Path::new(file);

        if !path.is_file() {
            log::error!("{} is not a file", file);
            std::process::exit(1);
        }

        let content_type = mime_guess::from_path(path)
            .first_raw()
            .unwrap_or("application/octet-stream");

        log::info!(
            "Insert file {} (content-type: {}) to FLUTE sender",
            file,
            content_type
        );

        let transfer_config = TransferConfig {
            max_transfer_count: cli.max_transfer_count,
            cenc,
            carousel_mode,
            ..Default::default()
        };

        let obj = CreateFromFile::builder()
            .path(path.to_path_buf())
            .content_type(content_type.to_string())
            .cache_in_ram(cache_in_ram)
            .compute_md5(compute_md5)
            .config(transfer_config)
            .build()
            .create()
            .unwrap();
        sender.add_object(0, obj).expect("Add object failed");
    }

    log::info!("Publish FDT update");
    sender.publish(SystemTime::now()).expect("Publish failed");

    // Send a "close session" packet to notify the receiver that the
    // previous session should be terminated, a new one is about to start.
    let close_session_pkt = sender.read_close_session(SystemTime::now());
    udp_socket.send(&close_session_pkt).expect("Send failed");

    let mut bucket = if cli.bitrate > 0 {
        let bps = cli.bitrate * 1000;
        // Burst size: allow up to 100ms worth of data
        let burst = std::cmp::max(bps / 80, 1500) as u64;
        Some(token_bucket::TokenBucket::new(
            bps,
            burst,
            cli.symbol_length,
        ))
    } else {
        None
    };

    loop {
        if let Some(ref mut b) = bucket {
            b.wait_for_capacity();
        }
        match sender.read(SystemTime::now()) {
            Some(pkt) => {
                if let Some(ref mut b) = bucket {
                    b.consume(pkt.len());
                }
                udp_socket.send(&pkt).expect("Send failed");
            }
            None => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
    }
}

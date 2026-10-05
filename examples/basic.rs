//! Minimal end-to-end example.
//!
//! Run against a live radar:
//! `cargo run --example basic 127.0.0.1:5002`

use fdad_radar::commands::OutgoingCommand;
use fdad_radar::control::ControlFrame;
use fdad_radar::crc::crc16_modbus;
use fdad_radar::data::DataFrame;
use std::time::Duration;

fn main() {
    // 1) Build and inspect a control frame (no network needed).
    let frame = ControlFrame::new(OutgoingCommand::Ctrl2A, &[0x00, 0x01, 0x02]);
    let wire = frame.encode();
    println!("control frame ({} bytes): {}", wire.len(), hex(&wire));
    println!("  decoded: {:?}", ControlFrame::decode(&wire).unwrap());
    println!("CRC-16/MODBUS of \"123456789\" = 0x{:04X}", crc16_modbus(b"123456789"));

    // 2) Build a synthetic data frame and parse it back.
    let payload = vec![0xAAu8; 8];
    let data = DataFrame::build(0x01, 0x00, 4, 1, 1_700_000_000_000, &payload);
    let parsed = DataFrame::parse(&data).unwrap();
    println!(
        "data frame type={:?} ts={} payload_len={}",
        parsed.header.kind(),
        parsed.header.timestamp,
        parsed.payload.len()
    );

    // 3) Talk to a real radar if an address was given.
    let Some(addr) = std::env::args().nth(1) else {
        println!("\n(pass <ip:port> to actually send/receive)");
        return;
    };
    match fdad_radar::transport::UdpRadar::connect(&addr) {
        Ok(radio) => {
            radio.set_read_timeout(Some(Duration::from_secs(2))).ok();
            println!("connected to {}", radio.peer());
            if let Err(e) = radio.send_control(&frame) {
                eprintln!("send failed: {e}");
                return;
            }
            let mut buf = [0u8; 65535];
            match radio.recv(&mut buf) {
                Ok(n) => {
                    println!("rx {n} bytes: {}", hex(&buf[..n.min(64)]));
                    match DataFrame::parse(&buf[..n]) {
                        Ok(f) => println!("  parsed: {:?}", f.header),
                        Err(e) => println!("  not a data frame: {e}"),
                    }
                }
                Err(e) => eprintln!("recv failed: {e}"),
            }
        }
        Err(e) => eprintln!("connect failed: {e}"),
    }
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02X}")).collect::<Vec<_>>().join(" ")
}

//! Live radar CLI — connect, heartbeat, receive + parse data, send control.
//!
//! ```text
//! cargo run --release --example radar_cli -- 192.168.8.167:5001 --secs 15
//! cargo run --release --example radar_cli -- --setreg 440:00030102
//! cargo run --release --example radar_cli -- --json '{"position":{"lat":29.8,"lon":31.08,"altitude":0.0,"yaw":0.0},"areas":[]}'
//! ```

use fdad_radar::client::RadarClient;
use fdad_radar::messages::{self, MessageType};
use std::time::{Duration, Instant};

fn main() {
    let mut addr = "192.168.8.167:5001".to_string();
    let mut secs = 15u64;
    let mut setreg: Option<String> = None;
    let mut json: Option<String> = None;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--secs" => { i += 1; secs = args.get(i).and_then(|s| s.parse().ok()).unwrap_or(15); }
            "--setreg" => { i += 1; setreg = args.get(i).cloned(); }
            "--json" => { i += 1; json = args.get(i).cloned(); }
            a if !a.starts_with("--") => addr = a.to_string(),
            _ => {}
        }
        i += 1;
    }

    println!("connecting to {addr} ...");
    let mut c = match RadarClient::connect(&addr, Some(Duration::from_millis(1500))) {
        Ok(c) => c,
        Err(e) => { eprintln!("connect failed: {e}"); return; }
    };
    println!("connected");

    if let Some(js) = &json {
        match c.set_json(js) { Ok(n) => println!("sent SetJson  (counter {n})"), Err(e) => eprintln!("set_json: {e}") }
    }
    if let Some(sr) = &setreg {
        let regs = parse_regs(sr);
        match c.set_register(&regs) { Ok(n) => println!("sent SetRegister {regs:?} (counter {n})"), Err(e) => eprintln!("set_register: {e}") }
    }

    let start = Instant::now();
    let mut last_hb = Instant::now() - Duration::from_secs(10);
    let (mut data, mut beat, mut other) = (0u64, 0u64, 0u64);
    let mut last_frame = 0u64;
    while start.elapsed() < Duration::from_secs(secs) {
        if last_hb.elapsed() >= Duration::from_secs(10) {
            if let Ok(n) = c.heartbeat() { println!("hb counter {n}"); }
            last_hb = Instant::now();
        }
        match c.recv() {
            Ok(msgs) => {
                for m in msgs {
                    match m.mtype {
                        MessageType::Data => {
                            data += 1;
                            if let Ok((h, rec)) = messages::parse_data(&m.body) {
                                if data <= 4 || h.frame_num != last_frame + 1 {
                                    println!(
                                        "DATA ctr={} frameNum={} ts={} bA={} bB={} rec={}B mac={:02x}:{:02x}:{:02x}:{:02x}:{:02x}:{:02x}",
                                        m.counter, h.frame_num, h.timestamp, h.boundary_a, h.boundary_b,
                                        rec.len(), h.mac[0], h.mac[1], h.mac[2], h.mac[3], h.mac[4], h.mac[5]
                                    );
                                }
                                last_frame = h.frame_num;
                            }
                        }
                        MessageType::Heartbeat => beat += 1,
                        _ => other += 1,
                    }
                }
            }
            Err(e) => eprintln!("recv: {e}"),
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    println!("summary: data={data} heartbeat={beat} other={other}");
}

fn parse_regs(s: &str) -> Vec<(u32, u32)> {
    s.split(',')
        .filter_map(|p| {
            let mut it = p.split(':');
            let r = u32::from_str_radix(it.next()?, 16).ok()?;
            let v = u32::from_str_radix(it.next()?, 16).ok()?;
            Some((r, v))
        })
        .collect()
}

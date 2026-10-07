# Using `fdad-radar` in a Tauri v2 + React/TypeScript app

The crate stays dependency-free by default. Enable the **`serde`** feature to get
`Serialize`/`Deserialize` (camelCase JSON) for the frontend.

## 1. Cargo.toml (your Tauri app)

```toml
[dependencies]
tauri = { version = "2", features = [] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
# enable the serialization feature:
fdad-radar = { path = "../fdad_radar", features = ["serde"] }
```

## 2. `src-tauri/src/lib.rs` — commands + live streaming

```rust
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use fdad_radar::{RadarClient, Sweep};
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Default)]
struct RadarState {
    client: Arc<Mutex<Option<RadarClient>>>,
    running: Arc<Mutex<bool>>,
}

/// Connect (auto-login) and start streaming sweeps to the frontend as
/// `radar://sweep` events. Idempotent: re-connecting drops any old session.
#[tauri::command]
fn radar_connect(app: AppHandle, state: State<'_, RadarState>, ip: String, port: u16) -> Result<(), String> {
    *state.running.lock().unwrap() = false;          // stop old reader
    *state.client.lock().unwrap() = None;

    let addr = format!("{ip}:{port}");
    let client = RadarClient::connect(&addr, Some(Duration::from_millis(100)))
        .map_err(|e| format!("connect failed: {e}"))?;
    *state.client.lock().unwrap() = Some(client);
    *state.running.lock().unwrap() = true;

    let client = state.client.clone();
    let running = state.running.clone();
    std::thread::spawn(move || {
        let mut last_hb = Instant::now() - Duration::from_secs(10);
        while *running.lock().unwrap() {
            let mut guard = client.lock().unwrap();
            if let Some(c) = guard.as_mut() {
                if last_hb.elapsed() >= Duration::from_secs(10) {
                    let _ = c.heartbeat();
                    last_hb = Instant::now();
                }
                if let Ok(sweeps) = c.recv_data() {
                    for s in sweeps {
                        let _ = app.emit("radar://sweep", &s);   // Sweep is Serialize
                    }
                }
            }
            drop(guard);
            std::thread::sleep(Duration::from_millis(10));
        }
    });
    Ok(())
}

/// Stop streaming and close the socket.
#[tauri::command]
fn radar_disconnect(state: State<'_, RadarState>) {
    *state.running.lock().unwrap() = false;
    *state.client.lock().unwrap() = None;
}

/// Start rotating (working-mode reg 0x401 = 0x401).
#[tauri::command]
fn radar_rotate(state: State<'_, RadarState>) -> Result<(), String> {
    if let Some(c) = state.client.lock().unwrap().as_mut() {
        c.rotate().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Stop rotating (reg 0x401 = 0).
#[tauri::command]
fn radar_stop(state: State<'_, RadarState>) -> Result<(), String> {
    if let Some(c) = state.client.lock().unwrap().as_mut() {
        c.stop().map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Arbitrary register write, e.g. `invoke("radar_set_register", { reg: 0x440, value: 0x00030102 })`.
#[tauri::command]
fn radar_set_register(state: State<'_, RadarState>, reg: u32, value: u32) -> Result<(), String> {
    if let Some(c) = state.client.lock().unwrap().as_mut() {
        c.set_register(&[(reg, value)]).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Settings JSON, e.g. the radar site position.
#[tauri::command]
fn radar_set_json(state: State<'_, RadarState>, json: String) -> Result<(), String> {
    if let Some(c) = state.client.lock().unwrap().as_mut() {
        c.set_json(&json).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(RadarState::default())
        .invoke_handler(tauri::generate_handler![
            radar_connect,
            radar_disconnect,
            radar_rotate,
            radar_stop,
            radar_set_register,
            radar_set_json,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

## 3. Frontend — React + TypeScript

Copy `frontend/radar.d.ts` into your app (e.g. `src/radar.ts`).

```tsx
// src/RadarView.tsx
import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { Sweep } from "./radar";       // the .d.ts

export default function RadarView() {
  const [connected, setConnected] = useState(false);
  const [sweep, setSweep] = useState<Sweep | null>(null);
  const [err, setErr] = useState<string | null>(null);

  // Live stream: the backend emits `radar://sweep` for every decoded sweep.
  useEffect(() => {
    let un: UnlistenFn | undefined;
    listen<Sweep>("radar://sweep", (e) => setSweep(e.payload)).then((f) => (un = f));
    return () => un?.();
  }, []);

  async function connect() {
    try {
      await invoke("radar_connect", { ip: "192.168.8.167", port: 5001 });
      setConnected(true); setErr(null);
    } catch (e) { setErr(String(e)); }
  }
  const disconnect = () => invoke("radar_disconnect").then(() => setConnected(false));
  const rotate = () => invoke("radar_rotate").catch((e) => setErr(String(e)));
  const stop = () => invoke("radar_stop").catch((e) => setErr(String(e)));

  return (
    <div>
      <div className="toolbar">
        {!connected
          ? <button onClick={connect}>Connect</button>
          : <button onClick={disconnect}>Disconnect</button>}
        <button onClick={rotate} disabled={!connected}>Rotate</button>
        <button onClick={stop} disabled={!connected}>Stop</button>
      </div>

      {err && <p style={{ color: "red" }}>{err}</p>}

      {sweep && (
        <p>
          sweep {sweep.boundaryADeg.toFixed(1)}–{sweep.boundaryBDeg.toFixed(1)}°
          · {sweep.targetCount} targets · frame {sweep.frameNum}
        </p>
      )}

      <svg viewBox="-1 -1 2 2" width={480} height={480}>
        <circle r="1" fill="none" stroke="#244" />
        {sweep?.targets.map((t) => {
          const r = Math.min(t.distanceM / 15000, 1);
          const a = (t.azimuthDeg * Math.PI) / 180;
          return <circle key={t.id} cx={r * Math.sin(a)} cy={-r * Math.cos(a)} r={0.02} fill="#fe4" />;
        })}
      </svg>
    </div>
  );
}
```

Make sure `run()` is called from `main.rs`:

```rust
fn main() { app_lib::run(); }
```

## Notes

- **camelCase JSON.** Because the types use `#[serde(rename_all = "camelCase")]`, TS sees
  `frameNum`, `boundaryADeg`, `distanceM`, `speedMps`, … (see `frontend/radar.d.ts`).
- **`Record.raw` is skipped** in JSON — use the flat `Target` (via `DataMessage::sweep()`)
  for rendering; the raw 64 bytes stay available in Rust.
- **Numbers.** `u64`/`i64` (`frameNum`, `timestamp`) are sent as JS numbers; the observed
  values (~1.7e12) are well within JS's `2^53` safe range.
- **Threading.** The reader thread holds a short (100 ms) read timeout so `rotate`/`stop`
  commands stay responsive. For higher throughput, move the socket to a dedicated thread
  and talk to it over an `mpsc` channel.
- Everything you need is re-exported at the crate root: `fdad_radar::{RadarClient, Sweep, Target, Record, DataMessage, DataHeader, Message, MessageType}`.

## Build / test

```sh
cargo test                      # 8 unit + 2 doc (no serde)
cargo test --features serde     # + frontend serialization test
cargo build --release
```


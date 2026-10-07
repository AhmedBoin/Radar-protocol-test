// Frontend types for the `fdad-radar` crate (serde output, camelCase).
// Enable the crate feature:  fdad-radar = { path = "...", features = ["serde"] }
// These mirror src/messages.rs (Target / Sweep / Record / DataHeader).

/** Message type tag (externally tagged enum). */
export type MessageType =
  | "heartbeat"
  | "login"
  | "setRegister"
  | "getRegister"
  | "setJson"
  | "getJson"
  | "data"
  | { other: number };

/** A raw message (`type | counter | body`). */
export interface Message {
  mtype: MessageType;
  counter: number;
  body: number[];
}

/** Sweep header as decoded from the wire. */
export interface DataHeader {
  /** 6-byte device MAC. */
  mac: number[];
  /** Sweep/message counter. */
  frameNum: number;
  /** Radar clock, milliseconds. */
  timestamp: number;
  /** Raw sector boundary A (divide by 10000 for degrees). */
  boundaryA: number;
  /** Raw sector boundary B (divide by 10000 for degrees). */
  boundaryB: number;
}

/** One raw 64-byte target record (tail bytes are not yet decoded). */
export interface Record {
  id: number;
  xCm: number;
  yCm: number;
  heightCm: number;
  vxCms: number;
  vyCms: number;
  kind: number;
  /** (snr*100 << 16) | (rcs*100). */
  packed: number;
}

/** Flat, render-ready target (raw fields + derived values). */
export interface Target {
  id: number;
  xCm: number;
  yCm: number;
  heightCm: number;
  vxCms: number;
  vyCms: number;
  kind: number;
  /** Ground distance, meters. */
  distanceM: number;
  /** Azimuth in degrees, 0 = north, clockwise. */
  azimuthDeg: number;
  /** Height, meters. */
  heightM: number;
  /** Ground speed, m/s. */
  speedMps: number;
  /** Heading in degrees, 0 = north. */
  headingDeg: number;
  /** SNR (value / 100). */
  snr: number;
  /** RCS (value / 100). */
  rcs: number;
}

/** A full sweep — the natural payload for a radar view. */
export interface Sweep {
  mac: number[];
  frameNum: number;
  timestamp: number;
  /** Sweep start azimuth, degrees. */
  boundaryADeg: number;
  /** Sweep end azimuth, degrees. */
  boundaryBDeg: number;
  targetCount: number;
  targets: Target[];
}

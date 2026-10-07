#!/usr/bin/env python3
"""radar_gui.py - small live GUI for the FDAD/BWR radar TCP protocol.

Connects to the radar (default 192.168.8.167:5001), heartbeats, receives the data
stream, shows the time base, and plots targets. Has controls to start/stop the radar
and change settings. Zero dependencies (tkinter + socket only).

Protocol (verified):
  FRAME : 55 AA 55 AA | LEN(u32 BE = payload+2) | PAYLOAD | CRC16-MODBUS(payload) LE
  HEARTBEAT : type=0x01 | counter(u32) | u64(0)
  SETREG    : type=0x40 | counter | count | (reg u32, value u32)*
  SETJSON   : type=0x80 | counter | len | 0 | json
  GETJSON   : type=0x82 | counter | ...
  DATA      : type=0x00030002 | counter | 0006 | MAC6 | frameNum(u64) | ts(i64 ms) |
              boundaryA(u32) | boundaryB(u32) | records
"""
import socket, struct, threading, queue, time, math, json
import tkinter as tk
from tkinter import ttk

MAGIC = b"\x55\xaa\x55\xaa"

def crc16_modbus(data: bytes) -> int:
    crc = 0xFFFF
    for x in data:
        crc ^= x
        for _ in range(8):
            crc = (crc >> 1) ^ 0xA001 if (crc & 1) else (crc >> 1)
    return crc

def build_frame(payload: bytes) -> bytes:
    return MAGIC + struct.pack(">I", len(payload) + 2) + payload + struct.pack("<H", crc16_modbus(payload))

def heartbeat(counter: int) -> bytes:
    return struct.pack(">IIQ", 0x01, counter, 0)

def set_register(counter: int, regs) -> bytes:
    b = struct.pack(">III", 0x40, counter, len(regs))
    for r, v in regs:
        b += struct.pack(">II", r & 0xFFFFFFFF, v & 0xFFFFFFFF)
    return b

def set_json(counter: int, obj) -> bytes:
    j = obj if isinstance(obj, (bytes, str)) else json.dumps(obj)
    jb = j.encode() if isinstance(j, str) else j
    return struct.pack(">IIII", 0x80, counter, len(jb), 0) + jb

def get_json(counter: int) -> bytes:
    return struct.pack(">II", 0x82, counter)


def login(counter: int) -> bytes:
    """LoginMessage (type 2). Body captured from the app at connect: 00000003 00000000."""
    return struct.pack(">II", 0x02, counter) + struct.pack(">II", 3, 0)


def get_register(counter: int, regs) -> bytes:
    """GetRegister (type 0x41): count + register ids."""
    b = struct.pack(">III", 0x41, counter, len(regs))
    for r in regs:
        b += struct.pack(">I", r & 0xFFFFFFFF)
    return b


class RadarLink:
    """Background TCP link: heartbeats + frame reassembly into a queue."""

    def __init__(self, log=print):
        self.sock = None
        self.running = False
        self.counter = 1
        self.lock = threading.Lock()
        self.q = queue.Queue()
        self.rx_frames = 0
        self.rx_bytes = 0
        self.crc_ok = 0
        self.crc_bad = 0
        self.log = log

    def connect(self, ip, port):
        self.sock = socket.create_connection((ip, port), timeout=3)
        self.sock.settimeout(1.0)
        self.running = True
        # LOGIN first (required before the radar streams)
        self.send(login(0))
        self.counter = 1
        threading.Thread(target=self._rx, daemon=True).start()
        threading.Thread(target=self._hb, daemon=True).start()

    def _ncounter(self):
        with self.lock:
            c = self.counter
            self.counter += 1
            return c

    def send(self, payload):
        if self.sock:
            self.sock.sendall(build_frame(payload))

    def heartbeat(self):
        self.send(heartbeat(self._ncounter()))

    def ctl_register(self, regs):
        self.send(set_register(self._ncounter(), regs))

    def ctl_json(self, obj):
        self.send(set_json(self._ncounter(), obj))

    def _hb(self):
        while self.running:
            try:
                self.heartbeat()
            except Exception:
                pass
            time.sleep(10)

    def _rx(self):
        buf = b""
        while self.running:
            try:
                d = self.sock.recv(65536)
            except socket.timeout:
                continue
            except OSError:
                break
            if not d:
                break
            self.rx_bytes += len(d)
            buf += d
            while True:
                i = buf.find(MAGIC)
                if i < 0:
                    buf = b""
                    break
                if i > 0:
                    buf = buf[i:]
                if len(buf) < 8:
                    break
                L = struct.unpack(">I", buf[4:8])[0]
                if len(buf) < 8 + L:
                    break
                frame, buf = buf[:8 + L], buf[8 + L:]
                pl, crc = frame[8:8 + L - 2], struct.unpack("<H", frame[8 + L - 2:8 + L])[0]
                if crc16_modbus(pl) == crc:
                    self.crc_ok += 1
                    self.rx_frames += 1
                    self.q.put(pl)
                else:
                    self.crc_bad += 1

    def close(self):
        self.running = False
        try:
            self.sock.close()
        except Exception:
            pass


def parse_records(rec: bytes):
    """Records area = u64 count, then count x 64-byte vehicle records.
    Each record (verified vs the DB/CSV):
      off 0  i32  X east  (cm)         off 12 i32 Vx east (cm/s)
      off 4  i32  Y north (cm)         off 16 i32 Vy north (cm/s)
      off 8  i32  Height  (cm)         off 20 i32 (type/menace)
      off 24 u32  ID (batch NO)        off 28 u32 (SNR*100 << 16 | RCS*100)
    Derived: distance=hypot(X,Y)/100 m, azimuth=atan2(X,Y) deg, speed=hypot(Vx,Vy)/100.
    """
    if len(rec) < 8:
        return []
    n = struct.unpack(">Q", rec[:8])[0]
    if n == 0 or n > 4096 or 8 + n * 64 > len(rec):
        return []
    out = []
    off = 8
    for _ in range(int(n)):
        r = rec[off:off + 64]
        off += 64
        X = struct.unpack(">i", r[0:4])[0]
        Y = struct.unpack(">i", r[4:8])[0]
        H = struct.unpack(">i", r[8:12])[0]
        Vx = struct.unpack(">i", r[12:16])[0]
        Vy = struct.unpack(">i", r[16:20])[0]
        tid = struct.unpack(">I", r[24:28])[0]
        pk = struct.unpack(">I", r[28:32])[0]
        out.append(dict(
            id=tid,
            dist=math.hypot(X, Y) / 100.0,
            az=math.degrees(math.atan2(X, Y)) % 360.0,
            height=H / 100.0,
            speed=math.hypot(Vx, Vy) / 100.0,
            snr=(pk >> 16) / 100.0,
            rcs=(pk & 0xFFFF) / 100.0,
        ))
    return out


def parse_data_header(body: bytes):
    if len(body) < 32:
        return None
    return dict(
        mac=body[2:8],
        frame_num=struct.unpack(">Q", body[8:16])[0],
        timestamp=struct.unpack(">q", body[16:24])[0],
        boundary_a=struct.unpack(">I", body[24:28])[0],
        boundary_b=struct.unpack(">I", body[28:32])[0],
        records=body[32:],
    )


class App:
    def __init__(self, root):
        self.root = root
        self.link = RadarLink(self._log)
        self.last_fn = None
        self.last_ts = None
        self.dt = None
        self.frames = 0
        self.t0 = time.time()
        self.tracks = {}          # id -> latest vehicle dict (persists so dots move, not duplicate)
        self.targets = []
        root.title("FDAD Radar - Protocol Test")
        root.geometry("1180x720")

        top = ttk.Frame(root, padding=6); top.pack(fill="x")
        ttk.Label(top, text="IP").pack(side="left")
        self.ip = ttk.Entry(top, width=16); self.ip.insert(0, "192.168.8.167"); self.ip.pack(side="left", padx=(2, 8))
        ttk.Label(top, text="Port").pack(side="left")
        self.port = ttk.Entry(top, width=6); self.port.insert(0, "5001"); self.port.pack(side="left", padx=(2, 8))
        self.btn_conn = ttk.Button(top, text="Connect", command=self.toggle_conn); self.btn_conn.pack(side="left")
        ttk.Button(top, text="Heartbeat", command=lambda: self.link.heartbeat()).pack(side="left", padx=4)
        ttk.Button(top, text="Get JSON", command=lambda: self.link.send(get_json(self.link._ncounter()))).pack(side="left", padx=4)
        self.status = ttk.Label(top, text="disconnected", foreground="red"); self.status.pack(side="left", padx=12)

        body = ttk.Frame(root); body.pack(fill="both", expand=True)

        left = ttk.LabelFrame(body, text="Radar control", padding=8); left.pack(side="left", fill="y", padx=6, pady=6)
        ttk.Label(left, text="Working mode").pack(anchor="w")
        self.mode = ttk.Combobox(left, values=["Cir sweep 1s", "Cir sweep 2s", "Cir sweep 3s", "Fan sweep", "Stand by"], width=16)
        self.mode.current(0); self.mode.pack(anchor="w", pady=2)
        ttk.Label(left, text="mode register (hex)").pack(anchor="w")
        self.reg_mode = ttk.Entry(left, width=10); self.reg_mode.insert(0, "440"); self.reg_mode.pack(anchor="w", pady=2)
        self.mode_val = ttk.Entry(left, width=16); self.mode_val.insert(0, "00030102"); self.mode_val.pack(anchor="w", pady=2)
        ttk.Button(left, text="Apply working mode", command=self.apply_mode).pack(fill="x", pady=3)

        ttk.Separator(left).pack(fill="x", pady=6)
        ttk.Label(left, text="Start / Stop (SetRegister)").pack(anchor="w")
        row = ttk.Frame(left); row.pack(anchor="w", pady=2)
        ttk.Label(row, text="reg").pack(side="left")
        self.reg_onoff = ttk.Entry(row, width=7); self.reg_onoff.insert(0, "401"); self.reg_onoff.pack(side="left")
        ttk.Label(row, text="on").pack(side="left")
        self.val_on = ttk.Entry(row, width=9); self.val_on.insert(0, "00000401"); self.val_on.pack(side="left")
        ttk.Label(row, text="off").pack(side="left")
        self.val_off = ttk.Entry(row, width=9); self.val_off.insert(0, "00000000"); self.val_off.pack(side="left")
        row2 = ttk.Frame(left); row2.pack(anchor="w", pady=3)
        ttk.Button(row2, text="Rotate", command=lambda: self.send_reg(self.reg_onoff.get(), self.val_on.get())).pack(side="left", padx=2)
        ttk.Button(row2, text="Stop", command=lambda: self.send_reg(self.reg_onoff.get(), self.val_off.get())).pack(side="left", padx=2)

        ttk.Separator(left).pack(fill="x", pady=6)
        ttk.Label(left, text="Position (SetJson)").pack(anchor="w")
        self.pos_entries = {}
        for name, val in (("lat", "29.807569"), ("lon", "31.081493"), ("alt", "0.0"), ("yaw", "-1.5102")):
            r = ttk.Frame(left); r.pack(anchor="w", pady=1)
            ttk.Label(r, text=name, width=4).pack(side="left")
            e = ttk.Entry(r, width=12); e.insert(0, val); e.pack(side="left"); self.pos_entries[name] = e
        ttk.Button(left, text="Send position", command=self.send_position).pack(fill="x", pady=3)

        ttk.Separator(left).pack(fill="x", pady=6)
        ttk.Label(left, text="Raw SetRegister  reg:value (hex)").pack(anchor="w")
        self.raw_reg = ttk.Entry(left, width=22); self.raw_reg.insert(0, "440:00030102"); self.raw_reg.pack(anchor="w", pady=2)
        ttk.Button(left, text="Send", command=self.send_raw_reg).pack(fill="x")

        right = ttk.Frame(body); right.pack(side="right", fill="both", expand=True)
        self.canvas = tk.Canvas(right, bg="#0b0f14", highlightthickness=0)
        self.canvas.pack(fill="both", expand=True, padx=6, pady=6)
        self.tb = ttk.Label(right, text="time base: -", font=("Consolas", 10)); self.tb.pack(anchor="w", padx=8)
        self.logbox = tk.Text(right, height=8, bg="#0b0f14", fg="#9fd", font=("Consolas", 9)); self.logbox.pack(fill="x", padx=6, pady=(0, 6))
        self.root.after(50, self.pump)

    def _log(self, s):
        self.logbox.insert("end", s + "\n"); self.logbox.see("end")

    def toggle_conn(self):
        if self.link.running:
            self.link.close(); self.link.running = False
            self.btn_conn.config(text="Connect"); self.status.config(text="disconnected", foreground="red")
        else:
            try:
                self.link.connect(self.ip.get(), int(self.port.get()))
                self.btn_conn.config(text="Disconnect"); self.status.config(text="connected", foreground="green")
                self._log("connected to %s:%s" % (self.ip.get(), self.port.get()))
            except Exception as e:
                self._log("connect failed: %s" % e)

    def send_reg(self, reg, val):
        try:
            self.link.ctl_register([(int(reg, 16), int(val, 16))])
            self._log("SetRegister reg=0x%s val=0x%s" % (reg, val))
        except Exception as e:
            self._log("setreg err: %s" % e)

    def send_raw_reg(self):
        try:
            reg, val = self.raw_reg.get().split(":")
            self.link.ctl_register([(int(reg, 16), int(val, 16))])
            self._log("SetRegister reg=0x%s val=0x%s" % (reg, val))
        except Exception as e:
            self._log("setreg err: %s" % e)

    def apply_mode(self):
        self.send_reg(self.reg_mode.get(), self.mode_val.get())

    def send_position(self):
        pe = self.pos_entries
        obj = {"position": {"lat": float(pe["lat"].get()), "lon": float(pe["lon"].get()),
                            "altitude": float(pe["alt"].get()), "yaw": float(pe["yaw"].get())}, "areas": []}
        try:
            self.link.ctl_json(obj); self._log("SetJson position sent")
        except Exception as e:
            self._log("setjson err: %s" % e)

    def update_tb(self):
        h = getattr(self, "hdr", None)
        el = time.time() - self.t0
        rate = self.frames / el if el > 0 else 0
        if h:
            self.tb.config(text=(
                "time base:  frameNum=%d   ts=%d ms   dt=%s ms   sweep=%.2f..%.2f deg   "
                "records=%dB   |   rx=%d  crc_ok=%d crc_bad=%d  %.1f msg/s   bytes=%d"
                % (h["frame_num"], h["timestamp"], self.dt,
                   h["boundary_a"] / 10000.0, h["boundary_b"] / 10000.0, len(h["records"]),
                   self.link.rx_frames, self.link.crc_ok, self.link.crc_bad, rate, self.link.rx_bytes)))
        else:
            self.tb.config(text="time base:  waiting for data...  rx=%d crc_ok=%d crc_bad=%d" %
                                (self.link.rx_frames, self.link.crc_ok, self.link.crc_bad))

    def plot(self):
        c = self.canvas; c.delete("all")
        w = c.winfo_width() or 700; hh = c.winfo_height() or 600
        cx, cy = w // 2, hh // 2
        R = min(cx, cy) - 20
        # rings
        for i in range(1, 6):
            r = R * i / 5
            c.create_oval(cx - r, cy - r, cx + r, cy + r, outline="#1b2a3a")
        for a in range(0, 360, 30):
            rad = math.radians(a)
            c.create_line(cx, cy, cx + R * math.sin(rad), cy - R * math.cos(rad), fill="#16324a")
            c.create_text(cx + (R + 12) * math.sin(rad), cy - (R + 12) * math.cos(rad), text=str(a), fill="#456")
        hdr = getattr(self, "hdr", None)
        if hdr:
            a0 = hdr["boundary_a"] / 10000.0
            a1 = hdr["boundary_b"] / 10000.0
            def pt(a, r):
                rad = math.radians(a)
                return cx + r * math.sin(rad), cy - r * math.cos(rad)
            p0 = pt(a0, R); p1 = pt(a1, R)
            c.create_polygon(cx, cy, p0[0], p0[1], p1[0], p1[1], fill="#12324a", outline="#3ad")
            c.create_text(w - 10, 12, anchor="ne",
                          text="sweep %.1f-%.1f deg" % (a0, a1), fill="#7cf")
            now = time.time()
            live = [t for t in self.tracks.values() if now - t.get("seen", 0) < 5.0]
            RANGE_MAX = 15000.0
            for t in live:
                az = t["az"]; dist = t["dist"]
                rr = R * min(dist / RANGE_MAX, 1.0)
                x, y = pt(az, rr)
                col = "#ffe14d" if t["snr"] > 0 else "#ff8a3d"
                c.create_oval(x - 4, y - 4, x + 4, y + 4, fill=col, outline="#a80")
                c.create_text(x + 7, y - 7, anchor="w",
                              text="%d  %.0fm  %.0f\u00b0  %.1fm/s  h%.0f  snr%.0f rcs%.2f" %
                                   (t["id"], dist, az, t["speed"], t["height"], t["snr"], t["rcs"]),
                              fill=col, font=("Consolas", 8))
            c.create_text(10, 12, anchor="nw", text="vehicles: %d" % len(live), fill="#ffe14d")

    def pump(self):
        drained = 0
        for _ in range(20000):
            try:
                pl = self.link.q.get_nowait()
            except queue.Empty:
                break
            drained += 1
            if len(pl) < 40:
                continue
            t = struct.unpack(">I", pl[:4])[0]
            if t == 0x00030002:
                h = parse_data_header(pl[8:])
                if h:
                    self.frames += 1
                    if self.last_ts is not None:
                        self.dt = h["timestamp"] - self.last_ts
                    self.last_ts = h["timestamp"]
                    self.last_fn = h["frame_num"]
                    self.hdr = h
                    recs = parse_records(h["records"])
                    if recs:
                        _now = time.time()
                        for _v in recs:
                            _v["seen"] = _now
                            self.tracks[_v["id"]] = _v      # replace old position -> dot moves
                        self.targets = recs
        self.update_tb()
        self.plot()
        self.root.after(50, self.pump)


def main():
    root = tk.Tk()
    App(root)
    root.mainloop()


if __name__ == "__main__":
    main()



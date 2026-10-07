/*
 radar_socketdump.js — Frida "man-in-the-middle at the socket layer".
 Dumps EVERY TCP+UDP send/recv/connect/bind of the radar app straight to a file
 (flushed live), so we capture both directions with no Wireshark and no config change.

 Output columns (tab-separated), in <OUT> (also echoed to console):
   epoch_ms  DIR  API  LEN  HEX  ASCII  PEER
   DIR = SEND | RECV | CONNECT | BIND | ACCEPT
   PEER = ip:port when known (connect/sendto/recvfrom), else '-'

 Attach:  frida -n FDAD-DCTv3.0.0_EN.exe -l radar_socketdump.js
*/
const OUT = "D:/Work/fdad-radar-protocol/capture/radar-live.tsv";

let f = null;
try { f = new File(OUT, "w"); } catch (e) { f = null; }

function ts() { return Date.now(); }
function exp(name) {
  try { if (Module.findGlobalExportByName) return Module.findGlobalExportByName(name); } catch (e) { }
  try { return Module.findExportByName(null, name); } catch (e) { }
  return null;
}
function sockaddr(a) {
  try {
    const fam = a.readU16();
    if (fam === 2) {                                  // AF_INET
      const port = (a.add(2).readU8() << 8) | a.add(3).readU8();
      const ip = [a.add(4).readU8(), a.add(5).readU8(), a.add(6).readU8(), a.add(7).readU8()].join('.');
      return ip + ':' + port;
    }
    if (fam === 23) return '[ipv6]';
  } catch (e) { }
  return '-';
}
function hd(p, l) {
  try {
    const b = new Uint8Array(p.readByteArray(l));
    let h = '', s = '';
    for (let i = 0; i < b.length; i++) {
      h += b[i].toString(16).padStart(2, '0');
      s += (b[i] >= 32 && b[i] < 127) ? String.fromCharCode(b[i]) : '.';
    }
    return [h, s];
  } catch (e) { return ['?', '?']; }
}
function w(line) {
  try { if (f) { f.write(line + "\n"); f.flush(); } } catch (e) { }
  console.log(line);
}
function emit(dir, api, ptr, len, peer) {
  if (!len || len <= 0) return;
  if (len > 16384) len = 16384;
  const r = hd(ptr, len);
  w([ts(), dir, api, len, r[0], r[1], peer || '-'].join('\t'));
}
function event(dir, api, peer) {
  w([ts(), dir, api, 0, '', '', peer || '-'].join('\t'));
}

// ---- BSD classic -----------------------------------------------------------
function hookSend(api) {
  const p = exp(api); if (!p) return;
  Interceptor.attach(p, {
    onEnter(a) { this.b = a[1]; this.l = a[2].toInt32(); this.peer = (api === 'sendto') ? sockaddr(a[4]) : '-'; },
    onLeave(ret) { const n = ret.toInt32(); if (n > 0) emit('SEND', api, this.b, Math.min(n, this.l), this.peer); }
  });
}
function hookRecv(api) {
  const p = exp(api); if (!p) return;
  Interceptor.attach(p, {
    onEnter(a) { this.b = a[1]; this.from = (api === 'recvfrom') ? a[4] : null; },
    onLeave(ret) { const n = ret.toInt32(); if (n > 0) emit('RECV', api, this.b, n, this.from ? sockaddr(this.from) : '-'); }
  });
}
['send', 'sendto'].forEach(hookSend);
['recv', 'recvfrom'].forEach(hookRecv);

// ---- Winsock overlapped (WSABUF { ULONG len; CHAR* buf; }) -----------------
function hookWSASend(api) {
  const p = exp(api); if (!p) return;
  Interceptor.attach(p, {
    onEnter(a) { this.bufs = a[1]; this.n = this.bufs.readU32(); this.buf = this.bufs.add(Process.pointerSize).readPointer(); this.peer = (api === 'WSASendTo') ? sockaddr(a[5]) : '-'; },
    onLeave(ret) { if (ret.toInt32() >= 0 && this.n > 0) emit('SEND', api, this.buf, this.n, this.peer); }
  });
}
function hookWSARecv(api) {
  const p = exp(api); if (!p) return;
  Interceptor.attach(p, {
    onEnter(a) { this.bufs = a[1]; this.np = a[3]; this.buf = this.bufs.add(Process.pointerSize).readPointer(); this.from = (api === 'WSARecvFrom') ? a[4] : null; },
    onLeave(ret) {
      if (ret.toInt32() >= 0 && this.np && !this.np.isNull()) {
        const n = this.np.readU32();
        if (n > 0) emit('RECV', api, this.buf, n, this.from ? sockaddr(this.from) : '-');
      }
    }
  });
}
['WSASend', 'WSASendTo'].forEach(hookWSASend);
['WSARecv', 'WSARecvFrom'].forEach(hookWSARecv);

// ---- connection topology ---------------------------------------------------
function hookConnect(api) {
  const p = exp(api); if (!p) return;
  Interceptor.attach(p, {
    onLeave(ret) { if (ret.toInt32() === 0) event('CONNECT', api, sockaddr(this.addr)); },
    onEnter(a) { this.addr = a[1]; }
  });
}
['connect', 'WSAConnect'].forEach(hookConnect);

function hookBind(api) {
  const p = exp(api); if (!p) return;
  Interceptor.attach(p, { onEnter(a) { const s = sockaddr(a[1]); if (s !== '-') event('BIND', api, s); } });
}
['bind'].forEach(hookBind);

console.log('[radar_socketdump] hooks installed -> ' + OUT);

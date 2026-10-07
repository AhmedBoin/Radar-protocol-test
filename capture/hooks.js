/*
 hooks.js — Frida function-level tracing of the radar protocol code paths.
 The binary is NO-ASLR (image base 0x400000), so these absolute VAs are valid live.

 It logs entry (args) and return of each hooked function to
 capture\radar-hooks.tsv (flushed) + console, columns:
   epoch_ms  KIND  NAME  DETAIL

 Hooks:
   0x4C4910  control-frame builder        (args: out QByteArray*, cmd, payload*)
   0x4CFBA0  data-frame dispatcher        (arg0 = frame buffer ptr)
   0x4D06E0  data receiver (UDP)          (called per datagram)
   0x904430  frame header decoder
   0x4A7D90  RadarController::onTcpClientReceiveRegisters
 Plus a memory peek helper to read the frame bytes when we know the pointer.

 Attach:  frida -n FDAD-DCTv3.0.0_EN.exe -l hooks.js
*/
(function () {
const OUT = "D:/Work/fdad-radar-protocol/capture/radar-hooks.tsv";
let f = null;
try { f = new File(OUT, "w"); } catch (e) { f = null; }
function w(cols) {
  const line = cols.join('\t');
  try { if (f) { f.write(line + "\n"); f.flush(); } } catch (e) { }
  console.log(line);
}
function now() { return Date.now(); }

function trace(addr, name) {
  try {
    Interceptor.attach(ptr(addr), {
      onEnter(a) {
        this.a = [a[0].toString(), a[1].toString(), a[2].toString(), a[3].toString()];
      },
      onLeave(r) {
        w([now(), 'RET', name, 'ret=' + r.toString() + ' args=' + this.a.join(',')]);
      }
    });
    w([now(), 'HOOK', name, 'installed@' + addr]);
  } catch (e) {
    w([now(), 'HOOK', name, 'ERR ' + e]);
  }
}

// --- protocol code paths (absolute VAs, no-ASLR) ---------------------------
trace('0x4C4910', 'buildControlFrame');
trace('0x4CFBA0', 'dataFrameDispatcher');
trace('0x4D06E0', 'dataReceiverUdp');
trace('0x904430', 'decodeFrameHeader');
trace('0x4A7D90', 'onTcpClientReceiveRegisters');

// --- helper: dump bytes at an address (call from the REPL) -----------------
//   hex(ptr('0x...'), 64)
globalThis.hex = function (p, n) {
  try { return hexdump(p, { length: n, ansi: false }); } catch (e) { return 'ERR ' + e; }
};

w([now(), 'READY', 'hooks.js', 'protocol tracing active']);
})();

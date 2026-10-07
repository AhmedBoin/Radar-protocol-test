#!/usr/bin/env python3
"""mem.py - read / inspect the (live) radar app's memory via Frida.

The app is no-ASLR (image base 0x400000), so absolute VAs from the disassembly are
valid at runtime.

Usage:
  python mem.py modules                         # list loaded modules (name base size)
  python mem.py range   <addr>                  # which module/range is this VA in
  python mem.py read    <addr> [len=256]        # hexdump memory at VA
  python mem.py cstr    <addr> [max=256]        # read a C string at VA
  python mem.py ptr     <addr> [count=1]        # read pointer(s) at VA
  python mem.py scan    <hexpattern> [max=16]   # scan main module for bytes (e.g. 7e7e)
  python mem.py dump    <addr> <len> <outfile>  # write raw bytes to a file

Env:  RADAR_PROC = process name (default FDAD-DCTv3.0.0_EN.exe)
"""
import sys, os, time, threading, binascii
import frida

try:
    sys.stdout.reconfigure(encoding='utf-8', errors='replace')
except Exception:
    pass

PROC = os.environ.get('RADAR_PROC', 'FDAD-DCTv3.0.0_EN.exe')


def collect(code, timeout=8.0):
    session = frida.attach(PROC)
    msgs, ev = [], threading.Event()
    script = session.create_script(code)

    def on_msg(m, d):
        msgs.append(m)
        p = m.get('payload')
        if isinstance(p, dict) and p.get('done'):
            ev.set()

    script.on('message', on_msg)
    script.load()
    ev.wait(timeout)
    try: script.unload()
    except Exception: pass
    session.detach()
    return msgs


def emit(msgs):
    for m in msgs:
        t = m.get('type')
        if t == 'send':
            p = m['payload']
            if isinstance(p, dict) and p.get('done'):
                continue
            print(p.get('text', p) if isinstance(p, dict) else p)
        elif t == 'error':
            print('ERROR:', m.get('description'))


def main():
    a = sys.argv[1:]
    if not a:
        print(__doc__); return
    cmd = a[0].lower()

    if cmd == 'modules':
        code = ("Process.enumerateModules().forEach(m=>send(m.name+'  '+m.base+'  size=0x'+m.size.toString(16)));"
                "send({done:true});")
    elif cmd == 'range':
        code = ("var r=Process.findRangeByAddress(ptr('%s'));"
                "send(r? JSON.stringify({base:r.base.toString(),size:r.size,prot:r.protection}) : 'not mapped');"
                "send({done:true});") % a[1]
    elif cmd == 'read':
        ln = int(a[2], 0) if len(a) > 2 else 256
        code = ("send({text: hexdump(ptr('%s'), {length:%d, ansi:false})}); send({done:true});") % (a[1], ln)
    elif cmd == 'cstr':
        mx = int(a[2], 0) if len(a) > 2 else 256
        code = ("try{ send({text: ptr('%s').readCString(%d)}); }catch(e){ send({text:'ERR '+e}); } send({done:true});") % (a[1], mx)
    elif cmd == 'ptr':
        cnt = int(a[2], 0) if len(a) > 2 else 1
        code = ("var p=ptr('%s'); var out=[]; for(var i=0;i<%d;i++){ out.push(p.add(i*4).readPointer().toString()); }"
                "send({text: out.join('\\n')}); send({done:true});") % (a[1], cnt)
    elif cmd == 'scan':
        mx = int(a[2], 0) if len(a) > 2 else 16
        code = ("var mod=Process.findModuleByName('%s')||Process.enumerateModules()[0];"
                "var hits=Memory.scanSync(mod.base, mod.size, '%s');"
                "send({text:'module='+mod.name+' hits='+hits.length+'\\n'+hits.slice(0,%d).map(function(h){return h.address.toString()+'  '+h.size;}).join('\\n')});"
                "send({done:true});") % (PROC, a[1], mx)
    elif cmd == 'dump':
        addr, ln, out = a[1], int(a[2], 0), a[3]
        code = ("var b=ptr('%s').readByteArray(%d); send({done:true}, b);") % (addr, ln)
        session = frida.attach(PROC)
        got = {}
        script = session.create_script(code)
        script.on('message', lambda m, d: (got.__setitem__('data', d), got.__setitem__('done', True)) if m.get('type') == 'send' else None)
        script.load()
        for _ in range(80):
            if got.get('done'): break
            time.sleep(0.1)
        if got.get('data'):
            open(out, 'wb').write(got['data'])
            print('wrote %d bytes -> %s' % (len(got['data']), out))
        session.detach()
        return
    else:
        print('unknown command', cmd); return

    emit(collect(code))


if __name__ == '__main__':
    main()

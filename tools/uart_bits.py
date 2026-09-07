#!/usr/bin/env python3
"""按位解码 VCD 里的 UART 线 (TX / RX), 打印每一帧每一位的采样时刻与电平, 可输出带逐位标注的 VCD。

用法:
  python tools/uart_bits.py examples/protocols.vcd uart.RX uart.TX --baud 115200
  python tools/uart_bits.py examples/protocols.vcd uart.RX_9600_8E1 --baud 9600 --format 8E1
  python tools/uart_bits.py examples/protocols.vcd uart.RX --baud 115200 --annotate rx_bits.vcd

原理 (与 wavAnaly 内置的 decode_uart 相同):
  1. 空闲为高, 看到下降沿当作起始位的起点 t0
  2. 半个位时间后确认仍为低 (排除毛刺)
  3. 在 t0 + (1.5 + k) * 位时间 处采样第 k 个数据位, LSB 先
  4. 校验位 (可选) 与停止位同样在位中点采样, 停止位必须为高
"""
import argparse
import sys

# Windows 控制台默认 GBK, 强制 UTF-8 输出
for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding='utf-8')
    except AttributeError:
        pass

# ---------------------------------------------------------------- VCD 读取
def read_vcd(path):
    """返回 (timescale_ns, {全路径: [(t, value_str), ...]})"""
    scope, id2names, values = [], {}, {}
    timescale_ns = 1.0
    cur_t = 0
    with open(path, encoding='utf-8', errors='replace') as f:
        header = True
        for raw in f:
            line = raw.strip()
            if not line:
                continue
            if header:
                toks = line.split()
                if toks[0] == '$timescale':
                    spec = ''.join(toks[1:]).replace('$end', '')
                    num = ''.join(ch for ch in spec if ch.isdigit()) or '1'
                    unit = spec[len(num):]
                    timescale_ns = int(num) * {'s': 1e9, 'ms': 1e6, 'us': 1e3, 'ns': 1, 'ps': 1e-3, 'fs': 1e-6}[unit]
                elif toks[0] == '$scope':
                    scope.append(toks[2])
                elif toks[0] == '$upscope':
                    scope.pop()
                elif toks[0] == '$var':
                    vid, name = toks[3], toks[4]
                    full = '.'.join(scope + [name])
                    id2names.setdefault(vid, []).append(full)
                    values.setdefault(full, [])
                elif toks[0] == '$enddefinitions':
                    header = False
                continue
            if line[0] == '#':
                cur_t = int(line[1:])
            elif line[0] in '01xzXZ':
                v, vid = line[0], line[1:]
                for full in id2names.get(vid, []):
                    values[full].append((cur_t, v))
            elif line[0] in 'bBrRsS':
                v, vid = line.split()
                for full in id2names.get(vid, []):
                    values[full].append((cur_t, v[1:]))
    return timescale_ns, values


def level_at(trace, t, idle=1):
    """trace: [(t, '0'/'1'/...)], 返回时刻 t 的电平 (最后一次 <= t 的值)"""
    lo, hi = 0, len(trace)
    while lo < hi:
        mid = (lo + hi) // 2
        if trace[mid][0] <= t:
            lo = mid + 1
        else:
            hi = mid
    if lo == 0:
        return idle
    v = trace[lo - 1][1]
    return 1 if v == '1' else 0 if v == '0' else idle


# ---------------------------------------------------------------- 逐位解码
def decode_bits(trace, bit_ns, data_bits=8, parity='N', stop_bits=1, inverted=False):
    """返回帧列表, 每帧: dict(t0, bits=[(名称, 采样时刻ns, 电平)], value, errors)"""
    idle = 0 if inverted else 1
    frames = []
    resume = 0.0
    for t, v in trace:
        lvl = 1 if v == '1' else 0
        if lvl == idle or t < resume:
            continue
        if level_at(trace, t + bit_ns * 0.5, idle) == idle:
            continue  # 毛刺
        bits = [('start', t + bit_ns * 0.5, level_at(trace, t + bit_ns * 0.5, idle))]
        value, ones = 0, 0
        for k in range(data_bits):
            ts = t + bit_ns * (1.5 + k)
            b = level_at(trace, ts, idle) ^ (1 if inverted else 0)
            bits.append((f'd{k}', ts, b))
            value |= b << k
            ones += b
        pos = 1 + data_bits
        errors = []
        if parity != 'N':
            ts = t + bit_ns * (pos + 0.5)
            pb = level_at(trace, ts, idle) ^ (1 if inverted else 0)
            bits.append(('parity', ts, pb))
            ones += pb
            if (parity == 'E' and ones % 2) or (parity == 'O' and not ones % 2):
                errors.append('PARITY')
            pos += 1
        for s in range(stop_bits):
            ts = t + bit_ns * (pos + 0.5 + s)
            sb = level_at(trace, ts, idle) ^ (1 if inverted else 0)
            bits.append((f'stop{s + 1 if stop_bits > 1 else ""}', ts, sb))
            if sb != 1:
                errors.append('FRAME')
        end = t + bit_ns * (pos + stop_bits)
        frames.append(dict(t0=t, end=end, bits=bits, value=value, errors=errors))
        resume = end - bit_ns * 0.25
    return frames


def printable(v):
    if 0x20 <= v < 0x7f:
        return repr(chr(v))
    return {0x0A: 'LF', 0x0D: 'CR', 0x09: 'TAB', 0x00: 'NUL'}.get(v, '')


def fmt_t(ns):
    return f'{ns / 1000:.2f} us' if ns < 1e6 else f'{ns / 1e6:.3f} ms'


# ---------------------------------------------------------------- 标注 VCD 输出
def write_annotated(path, timescale_ns, sig_frames, traces):
    """sig_frames: {信号名: frames}; 每个信号输出 原始线 + bit 标注 + byte 标注"""
    ids = {}
    n = 0

    def vid():
        nonlocal n
        s, m = '', n
        n += 1
        while True:
            s = chr(33 + m % 94) + s
            m = m // 94 - 1
            if m < 0:
                return s

    changes = []
    with open(path, 'w', encoding='utf-8') as f:
        f.write('$comment uart_bits.py annotated $end\n')
        f.write(f'$timescale {int(timescale_ns) if timescale_ns >= 1 else 1}ns $end\n')
        f.write('$scope module uart_bits $end\n')
        for sig in sig_frames:
            short = sig.split('.')[-1]
            ids[sig] = (vid(), vid(), vid())
            f.write(f'$var wire 1 {ids[sig][0]} {short} $end\n')
            f.write(f'$var string 1 {ids[sig][1]} {short}_bit $end\n')
            f.write(f'$var string 1 {ids[sig][2]} {short}_byte $end\n')
        f.write('$upscope $end\n$enddefinitions $end\n')
        for sig, frames in sig_frames.items():
            w, b, y = ids[sig]
            for t, v in traces[sig]:
                changes.append((t, f'{v}{w}'))
            changes.append((0, f'sidle {b}'))
            changes.append((0, f's- {y}'))
            for fr in frames:
                text = f'0x{fr["value"]:02X}'
                p = printable(fr['value']).replace("'", '')
                if p:
                    text += '_' + p
                if fr['errors']:
                    text += '_' + '/'.join(fr['errors']) + '?'
                changes.append((int(fr['t0']), f's{text} {y}'))
                changes.append((int(round(fr['end'])), f's- {y}'))
                # 位标注从每位起点开始 (采样点减半个位)
                half = (fr['bits'][1][1] - fr['bits'][0][1]) / 2 if len(fr['bits']) > 1 else 0
                for name, ts, lvl in fr['bits']:
                    changes.append((int(round(ts - half)), f's{name}={lvl} {b}'))
                changes.append((int(round(fr['end'])), f'sidle {b}'))
        changes.sort(key=lambda c: c[0])
        cur = None
        for t, line in changes:
            if t != cur:
                f.write(f'#{t}\n')
                cur = t
            f.write(line + '\n')
        last = max((c[0] for c in changes), default=0)
        f.write(f'#{last + 1}\n')


# ---------------------------------------------------------------- 主程序
def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('vcd')
    ap.add_argument('signals', nargs='+', help='要解码的线, 完整路径, 如 uart.RX uart.TX')
    ap.add_argument('--baud', type=float, default=115200)
    ap.add_argument('--format', default='8N1', help='数据位/校验/停止位, 如 8N1 7E2')
    ap.add_argument('--inverted', action='store_true', help='线路反相 (空闲为低)')
    ap.add_argument('--annotate', metavar='OUT.vcd', help='输出带逐位标注的 VCD')
    ap.add_argument('--brief', action='store_true', help='只打印字节, 不打印每一位')
    args = ap.parse_args()

    data_bits, parity, stop_bits = int(args.format[0]), args.format[1].upper(), int(args.format[2])
    timescale_ns, values = read_vcd(args.vcd)
    bit_units = 1e9 / args.baud / timescale_ns   # 一位占多少个 VCD 时间单位
    print(f'{args.vcd}: 时基 {timescale_ns:g} ns, 波特率 {args.baud:g} -> 一位 {1e9 / args.baud:.2f} ns, 格式 {args.format}\n')

    sig_frames = {}
    for sig in args.signals:
        if sig not in values:
            cand = [k for k in values if k.endswith('.' + sig) or k == sig]
            if len(cand) == 1:
                sig = cand[0]
            else:
                print(f'找不到信号 {sig}; 可用: {", ".join(sorted(values))}', file=sys.stderr)
                sys.exit(1)
        frames = decode_bits(values[sig], bit_units, data_bits, parity, stop_bits, args.inverted)
        sig_frames[sig] = frames
        print(f'=== {sig}: {len(frames)} 帧')
        text = ''
        for i, fr in enumerate(frames):
            ch = printable(fr['value'])
            err = ' ' + ' '.join(e + '?' for e in fr['errors']) if fr['errors'] else ''
            print(f'  帧 {i + 1:3d}  起始 {fmt_t(fr["t0"] * timescale_ns):>12}  '
                  f'-> 0x{fr["value"]:02X} {ch}{err}')
            if not args.brief:
                for name, ts, lvl in fr['bits']:
                    print(f'         {name:>7} @ {fmt_t(ts * timescale_ns):>12}  = {lvl}')
            if 0x20 <= fr['value'] < 0x7f:
                text += chr(fr['value'])
            elif fr['value'] in (0x0A, 0x0D):
                text += '\\n' if fr['value'] == 0x0A else '\\r'
        print(f'  文本: "{text}"\n')

    if args.annotate:
        write_annotated(args.annotate, timescale_ns, sig_frames, values)
        print(f'标注波形已写入 {args.annotate}, 用 wavanaly 打开后 scope_add uart_bits 查看')


if __name__ == '__main__':
    main()

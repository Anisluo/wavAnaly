#!/usr/bin/env python3
"""生成 PCIe Gen1 x1 单方向 lane 的比特流测试波形 examples/pcie_gen1.vcd (时基 100 ps)。

物理层: 2.5 GT/s, 一位 (UI) 400 ps, 8b/10b 编码, 串行发送顺序 a b c d e i f g h j。
链路内容 (RC -> EP 方向):
  逻辑空闲 (D0.0) -> SKP 有序集 (COM SKP SKP SKP) -> MWr32 TLP (1 DW 数据, seq 0)
  -> 空闲 -> MRd32 TLP (seq 1) -> Ack DLLP (AckNak_Seq 1) -> SKP 有序集 -> 空闲
扰码: 默认开启 (Gen1/Gen2 LFSR X^16+X^5+X^4+X^3+1, COM 复位, SKP 不推进), 加 --noscramble 关闭。
信号:
  pcie.TX_P / pcie.TX_N   差分对 (TX_N = 反相)
  pcie.REFCLK_100M        参考时钟 (与数据无相位关系, 只作对照)
  pcie.ref                真值标注: 当前符号 / 帧, 用来核对解码器
  pcie.symbol             线上的 8b/10b 符号 (加扰后), '>0xNN' 是扰码前的原始字节

CRC 按 PCIe Base Spec 的公式实现 (LCRC: CRC-32 04C11DB7, DLLP: CRC-16 100B, 均取反并按位反转),
解码器用同样的算法核对, 未用真实设备抓包比对过。
"""
import os

UI = 4                      # 400 ps / 100 ps 时基
TIMESCALE = '100ps'

# ---------------------------------------------------------------- 8b/10b 表
# 5b/6b: x -> (RD- 用, RD+ 用)  串行顺序 abcdei
D5B6B = {
    0: ('100111', '011000'), 1: ('011101', '100010'), 2: ('101101', '010010'), 3: ('110001', '110001'),
    4: ('110101', '001010'), 5: ('101001', '101001'), 6: ('011001', '011001'), 7: ('111000', '000111'),
    8: ('111001', '000110'), 9: ('100101', '100101'), 10: ('010101', '010101'), 11: ('110100', '110100'),
    12: ('001101', '001101'), 13: ('101100', '101100'), 14: ('011100', '011100'), 15: ('010111', '101000'),
    16: ('011011', '100100'), 17: ('100011', '100011'), 18: ('010011', '010011'), 19: ('110010', '110010'),
    20: ('001011', '001011'), 21: ('101010', '101010'), 22: ('011010', '011010'), 23: ('111010', '000101'),
    24: ('110011', '001100'), 25: ('100110', '100110'), 26: ('010110', '010110'), 27: ('110110', '001001'),
    28: ('001110', '001110'), 29: ('101110', '010001'), 30: ('011110', '100001'), 31: ('101011', '010100'),
}
# 3b/4b: y -> (RD-, RD+)  串行顺序 fghj ; 7 有主/备两种
D3B4B = {
    0: ('1011', '0100'), 1: ('1001', '1001'), 2: ('0101', '0101'), 3: ('1100', '0011'),
    4: ('1101', '0010'), 5: ('1010', '1010'), 6: ('0110', '0110'), 7: ('1110', '0001'),
}
D3B4B_ALT7 = ('0111', '1000')
K5B6B = {28: ('001111', '110000'), 23: ('111010', '000101'), 27: ('110110', '001001'),
         29: ('101110', '010001'), 30: ('011110', '100001')}
K3B4B = {0: ('1011', '0100'), 1: ('0110', '1001'), 2: ('1010', '0101'), 3: ('1100', '0011'),
         4: ('1101', '0010'), 5: ('0101', '1010'), 6: ('1001', '0110'), 7: ('0111', '1000')}


def disparity(bits):
    return bits.count('1') - bits.count('0')


class Encoder8b10b:
    def __init__(self):
        self.rd = -1

    def encode(self, byte, k=False):
        x, y = byte & 0x1F, byte >> 5
        if k:
            six = K5B6B[x][0 if self.rd < 0 else 1]
        else:
            six = D5B6B[x][0 if self.rd < 0 else 1]
        rd = self.rd + disparity(six) if disparity(six) else self.rd
        if k:
            four = K3B4B[y][0 if rd < 0 else 1]
        else:
            if y == 7 and ((rd < 0 and x in (17, 18, 20)) or (rd > 0 and x in (11, 13, 14))):
                four = D3B4B_ALT7[0 if rd < 0 else 1]
            else:
                four = D3B4B[y][0 if rd < 0 else 1]
        rd = rd + disparity(four) if disparity(four) else rd
        self.rd = rd
        return six + four            # 10 个字符, 发送顺序从左到右


# ---------------------------------------------------------------- CRC
def crc_generic(data, poly, width, init):
    crc = init
    top = 1 << (width - 1)
    mask = (1 << width) - 1
    for b in data:
        for i in range(7, -1, -1):
            bit = (b >> i) & 1
            fb = ((crc >> (width - 1)) & 1) ^ bit
            crc = ((crc << 1) & mask) | 0
            if fb:
                crc ^= poly
    return crc & mask


def reflect(v, width):
    r = 0
    for i in range(width):
        if v & (1 << i):
            r |= 1 << (width - 1 - i)
    return r


def lcrc32(data):
    """PCIe LCRC: CRC-32 04C11DB7, init FFFFFFFF, 结果取反, 位序反转"""
    c = crc_generic(data, 0x04C11DB7, 32, 0xFFFFFFFF)
    c = (~c) & 0xFFFFFFFF
    return reflect(c, 32)


def dllp_crc16(data):
    """PCIe DLLP CRC-16: 多项式 100B, init FFFF, 结果取反, 位序反转"""
    c = crc_generic(data, 0x100B, 16, 0xFFFF)
    c = (~c) & 0xFFFF
    return reflect(c, 16)


# ---------------------------------------------------------------- 符号流
K_COM, K_SKP, K_STP, K_SDP, K_END, K_PAD = 0xBC, 0x1C, 0xFB, 0x5C, 0xFD, 0xF7
symbols = []   # (byte, is_k, label)


def emit(byte, k=False, label=''):
    symbols.append((byte, k, label))


def idle(n):
    for _ in range(n):
        emit(0x00, False, 'idle')


def skp_os():
    emit(K_COM, True, 'COM')
    for _ in range(3):
        emit(K_SKP, True, 'SKP')


def tlp(seq, header, data=()):
    body = [(seq >> 8) & 0x0F, seq & 0xFF] + list(header) + list(data)
    crc = lcrc32(body)
    emit(K_STP, True, 'STP')
    for i, b in enumerate(body):
        lab = 'SEQ' if i < 2 else ('HDR' if i < 2 + len(header) else 'DATA')
        emit(b, False, lab)
    for sh in (24, 16, 8, 0):
        emit((crc >> sh) & 0xFF, False, 'LCRC')
    emit(K_END, True, 'END')


def dllp(payload4):
    crc = dllp_crc16(payload4)
    emit(K_SDP, True, 'SDP')
    for b in payload4:
        emit(b, False, 'DLLP')
    emit((crc >> 8) & 0xFF, False, 'CRC16')
    emit(crc & 0xFF, False, 'CRC16')
    emit(K_END, True, 'END')


def tlp_header_3dw(fmt, typ, length, req_id, tag, be, addr):
    b0 = (fmt << 5) | typ
    b1 = 0                       # TC=0, attrs=0
    b2 = (length >> 8) & 0x03    # TD=0 EP=0 attr=0
    b3 = length & 0xFF
    return [b0, b1, b2, b3, (req_id >> 8) & 0xFF, req_id & 0xFF, tag, be,
            (addr >> 24) & 0xFF, (addr >> 16) & 0xFF, (addr >> 8) & 0xFF, addr & 0xFC]


idle(12)
skp_os()
idle(4)
# MWr32: fmt=2 (3DW + data), type=0, length=1 DW, requester 00:00.0, tag 3, BE first=0xF, addr 0xF0001000
tlp(0, tlp_header_3dw(2, 0, 1, 0x0000, 0x03, 0x0F, 0xF0001000), data=[0xDE, 0xAD, 0xBE, 0xEF])
idle(6)
# MRd32: fmt=0 (3DW no data), type=0, length=1, tag 4, addr 0xF0001004
tlp(1, tlp_header_3dw(0, 0, 1, 0x0000, 0x04, 0x0F, 0xF0001004))
idle(3)
# Ack DLLP: type 0x00, AckNak_Seq = 1
dllp([0x00, 0x00, 0x00, 0x01])
idle(4)
skp_os()
idle(12)

# ---------------------------------------------------------------- 扰码 (Gen1/Gen2)
class Scrambler:
    """X^16+X^5+X^4+X^3+1, Galois 左移, 反馈掩码 0x39, 输出取 D15 (移位前), 字节 LSB 先。
    COM 复位为 0xFFFF; SKP 不推进; K 码与有序集内的 D 码不加扰但推进 LFSR。
    复位后对 D0.0 的扰码输出: FF 17 C0 14 B2 E7 02 82 ... (与规范一致)"""
    def __init__(self):
        self.lfsr = 0xFFFF
        self.in_ts = 0          # 剩余的训练序列符号数 (不加扰)

    def advance_byte(self):
        out = 0
        for i in range(8):
            msb = (self.lfsr >> 15) & 1
            out |= msb << i
            self.lfsr = (self.lfsr << 1) & 0xFFFF
            if msb:
                self.lfsr ^= 0x39
        return out

    def process(self, byte, k, next_is_d):
        if k:
            if byte == K_COM:
                self.lfsr = 0xFFFF
                self.in_ts = 15 if next_is_d else 0   # COM 后紧跟 D 码 = TS1/TS2 (16 符号)
                return byte
            if byte == K_SKP:
                return byte                            # SKP 不推进
            self.advance_byte()
            return byte
        mask = self.advance_byte()
        if self.in_ts:
            self.in_ts -= 1
            return byte
        return byte ^ mask


SCRAMBLE = '--noscramble' not in __import__('sys').argv

# ---------------------------------------------------------------- 输出 VCD
enc = Encoder8b10b()
scr = Scrambler()
out = os.path.join(os.path.dirname(__file__), '..', 'examples', 'pcie_gen1.vcd')
with open(out, 'w', encoding='utf-8') as f:
    f.write('$comment wavAnaly PCIe Gen1 x1 lane test waveform (8b/10b, 2.5 GT/s) $end\n')
    f.write(f'$timescale {TIMESCALE} $end\n$scope module pcie $end\n')
    f.write('$var wire 1 ! TX_P $end\n$var wire 1 " TX_N $end\n$var wire 1 # REFCLK_100M $end\n')
    f.write('$var string 1 $ ref $end\n$var string 1 % symbol $end\n$upscope $end\n$enddefinitions $end\n')
    changes = []
    t = 0
    last_bit = None
    last_ref = None
    for idx, (byte, k, label) in enumerate(symbols):
        wire_byte = byte
        if SCRAMBLE:
            next_is_d = idx + 1 < len(symbols) and not symbols[idx + 1][1]
            wire_byte = scr.process(byte, k, next_is_d)
        code = enc.encode(wire_byte, k)
        name = (f'K{wire_byte & 0x1F}.{wire_byte >> 5}' if k else f'D{wire_byte & 0x1F}.{wire_byte >> 5}')
        tag = f'{name}={code}' if (k or wire_byte == byte) else f'{name}={code}>0x{byte:02X}'
        changes.append((t, f's{tag} %'))
        if label != last_ref:
            changes.append((t, f's{label} $'))
            last_ref = label
        for ch in code:
            bit = int(ch)
            if bit != last_bit:
                changes.append((t, f'{bit}!'))
                changes.append((t, f'{1 - bit}"'))
                last_bit = bit
            t += UI
    # 100 MHz 参考时钟, 周期 10 ns = 100 单位
    tc = 0
    while tc < t:
        changes.append((tc, '1#'))
        changes.append((tc + 50, '0#'))
        tc += 100
    changes.sort(key=lambda c: c[0])
    cur = None
    for tt, line in changes:
        if tt != cur:
            f.write(f'#{tt}\n'); cur = tt
        f.write(line + '\n')
    f.write(f'#{t}\n')
print('wrote', os.path.abspath(out), len(symbols), 'symbols,', t / 10, 'ns')

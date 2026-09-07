#!/usr/bin/env python3
"""生成 I2C / SPI / UART 三种总线的测试波形 examples/protocols.vcd (时基 1 ns, 共 1.2 ms)。

  i2c  : 400 kHz, 主机读 AS5600 (0x36) 寄存器 0x0C/0x0D, 含 START / 重复 START / ACK / NACK / STOP
  spi  : 模式 0, 1 MHz, 片选低有效, 主机写 MAX7219 风格的 16 位帧 (地址+数据), MISO 回读
  uart : 115200 8N1, RX 收 "M6.28\\n", TX 发 "OK\\r\\n", 另有一帧 9600 8E1 演示校验
"""
import os

# ------------------------------------------------------------ VCD writer
class VCD:
    def __init__(self):
        self.vars, self.changes, self._n, self._last = [], [], 0, {}

    def _id(self):
        n, s = self._n, ''
        self._n += 1
        while True:
            s = chr(33 + n % 94) + s
            n = n // 94 - 1
            if n < 0:
                return s

    def wire(self, scope, name, width=1):
        i = self._id(); self.vars.append((scope, name, 'wire', width, i)); return i

    def string(self, scope, name):
        i = self._id(); self.vars.append((scope, name, 'string', 1, i)); return i

    def set(self, t, i, v):
        t = int(round(t))
        if self._last.get(i) == v and t != 0:
            return
        self._last[i] = v
        self.changes.append((t, i, v))

    def write(self, path, end):
        self.changes.sort(key=lambda c: (c[0], c[1]))
        kind = {v[4]: v[2] for v in self.vars}
        width = {v[4]: v[3] for v in self.vars}
        with open(path, 'w', encoding='utf-8') as f:
            f.write('$comment wavAnaly protocol test waveform (I2C / SPI / UART) $end\n$timescale 1ns $end\n')
            for sc in dict.fromkeys(v[0] for v in self.vars):
                f.write(f'$scope module {sc} $end\n')
                for scope, name, k, w, i in self.vars:
                    if scope == sc:
                        f.write(f'$var {k} {w} {i} {name} $end\n')
                f.write('$upscope $end\n')
            f.write('$enddefinitions $end\n')
            cur = None
            for t, i, v in self.changes:
                if t != cur:
                    f.write(f'#{t}\n'); cur = t
                if kind[i] == 'string':
                    f.write(f's{str(v).replace(" ", "_")} {i}\n')
                elif width[i] == 1:
                    f.write(f'{v}{i}\n')
                else:
                    f.write(f'b{int(v):b} {i}\n')
            f.write(f'#{end}\n')

vcd = VCD()
US = 1000
END = 1200 * US

# ------------------------------------------------------------ I2C 400 kHz
T = 2500; T_HIGH = 833; T_LOW = T - T_HIGH; HD_DAT = 300; SLV = 450
scl = vcd.wire('i2c', 'SCL'); sda = vcd.wire('i2c', 'SDA'); ref = vcd.string('i2c', 'ref')
vcd.set(0, scl, 1); vcd.set(0, sda, 1); vcd.set(0, ref, 'idle')

def i2c_bit(t, val):
    vcd.set(t + HD_DAT, sda, val); vcd.set(t + T_LOW, scl, 1); vcd.set(t + T_LOW + T_HIGH, scl, 0)
    return t + T

def i2c_byte(t, byte, label, ack):
    vcd.set(t, ref, label)
    for k in range(7, -1, -1):
        t = i2c_bit(t, (byte >> k) & 1)
    vcd.set(t, ref, 'ACK' if ack else 'NACK')
    vcd.set(t + HD_DAT, sda, 1)
    if ack:
        vcd.set(t + HD_DAT + SLV, sda, 0)
    vcd.set(t + T_LOW, scl, 1); vcd.set(t + T_LOW + T_HIGH, scl, 0)
    return t + T

def i2c_start(t, repeated):
    if repeated:
        vcd.set(t + HD_DAT, sda, 1); vcd.set(t + T_LOW, scl, 1); t += T_LOW + 600
    vcd.set(t, ref, 'Sr' if repeated else 'S'); vcd.set(t, sda, 0); vcd.set(t + 600, scl, 0)
    return t + 600

def i2c_stop(t):
    vcd.set(t + HD_DAT, sda, 0); vcd.set(t + T_LOW, scl, 1); vcd.set(t + T_LOW, ref, 'P')
    t += T_LOW + 600; vcd.set(t, sda, 1); vcd.set(t + 50, ref, 'idle')
    return t + 1300

def as5600_read(t, raw):
    t = i2c_start(t, False)
    t = i2c_byte(t, 0x6C, 'ADDR_W', True)
    t = i2c_byte(t, 0x0C, 'REG', True)
    t = i2c_start(t, True)
    t = i2c_byte(t, 0x6D, 'ADDR_R', True)
    t = i2c_byte(t, (raw >> 8) & 0x0F, 'DATA_H', True)   # 从机发, 主机 ACK (这里简化为同一函数)
    t = i2c_byte(t, raw & 0xFF, 'DATA_L', False)
    return i2c_stop(t)

t = 5 * US
raw = 0x0A3F
while t < 400 * US:
    t = as5600_read(t, raw); raw = (raw + 37) & 0xFFF
# 一次地址错误 (无人应答) 的事务
t = i2c_start(t + 20 * US, False)
t = i2c_byte(t, 0x6E, 'ADDR_W_bad', False)
i2c_stop(t)

# ------------------------------------------------------------ SPI 模式 0, 1 MHz
HALF = 500
sclk = vcd.wire('spi', 'SCLK'); mosi = vcd.wire('spi', 'MOSI'); miso = vcd.wire('spi', 'MISO'); cs = vcd.wire('spi', 'CS_n')
spi_ref = vcd.string('spi', 'ref')
vcd.set(0, sclk, 0); vcd.set(0, mosi, 0); vcd.set(0, miso, 0); vcd.set(0, cs, 1); vcd.set(0, spi_ref, 'idle')

def spi_frame(t, words, label):
    vcd.set(t, cs, 0); vcd.set(t, spi_ref, label)
    t += 2 * HALF
    for wo, wi in words:
        for k in range(15, -1, -1):
            vcd.set(t, mosi, (wo >> k) & 1); vcd.set(t + 60, miso, (wi >> k) & 1)   # 数据在下降沿后变化
            vcd.set(t + HALF, sclk, 1)                                                # 上升沿采样
            vcd.set(t + 2 * HALF, sclk, 0)
            t += 2 * HALF
    t += HALF
    vcd.set(t, cs, 1); vcd.set(t + 50, spi_ref, 'idle')
    return t + 20 * US

t = 10 * US
regs = [(0x0C01, 0x0000), (0x0900, 0x0C01), (0x0A08, 0x0900), (0x0B07, 0x0A08), (0x0107, 0x0B07), (0x0206, 0x0107)]
for i, (wo, wi) in enumerate(regs):
    t = spi_frame(t, [(wo, wi)], f'write_{wo >> 8:02X}')
# 一次连续多字帧
t = spi_frame(t, [(0x0101, 0xAA55), (0x0202, 0x5AA5), (0x0303, 0x1234)], 'burst')

# ------------------------------------------------------------ UART
BIT = 1e9 / 115200
tx = vcd.wire('uart', 'TX'); rx = vcd.wire('uart', 'RX'); rx2 = vcd.wire('uart', 'RX_9600_8E1'); uref = vcd.string('uart', 'ref')
vcd.set(0, tx, 1); vcd.set(0, rx, 1); vcd.set(0, rx2, 1); vcd.set(0, uref, 'idle')

def uart_char(t, line, ch, bit=BIT, parity=None):
    code = ord(ch)
    vcd.set(t, uref, {'\n': 'LF', '\r': 'CR', '.': 'dot'}.get(ch, ch))
    vcd.set(t, line, 0)
    ones = 0
    for k in range(8):
        b = (code >> k) & 1; ones += b
        vcd.set(t + (k + 1) * bit, line, b)
    n = 9
    if parity == 'E':
        vcd.set(t + n * bit, line, ones % 2); n += 1
    vcd.set(t + n * bit, line, 1)
    t += (n + 1) * bit
    vcd.set(t, uref, 'idle')
    return t

t = 50 * US
for ch in 'M6.28\n':
    t = uart_char(t, rx, ch)
t = 700 * US
for ch in 'OK\r\n':
    t = uart_char(t, tx, ch)
t = 200 * US
for ch in 'Hi':
    t = uart_char(t, rx2, ch, bit=1e9 / 9600, parity='E')

out = os.path.join(os.path.dirname(__file__), '..', 'examples', 'protocols.vcd')
vcd.write(out, END)
print('wrote', os.path.abspath(out), len(vcd.changes), 'changes')

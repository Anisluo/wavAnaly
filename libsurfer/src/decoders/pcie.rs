//! PCIe 物理层 / 数据链路层 / 事务层解码 (单 lane, 8b/10b, Gen1/Gen2)。
//!
//! 输入: 一根 lane 的单端或差分正端 (TX_P) 比特流。参数: `gen1` (2.5 GT/s, 默认) / `gen2` (5 GT/s)
//! / `ui=<ps>` 自定义位宽 / `noscramble` 不解扰 (默认按 Gen1/Gen2 规则解扰)。
//! 输出两条信号:
//! * `<名字>_sym` — 每个 10 位符号: `K28.5 COM`, `D10.2 0x4A`, 未能解码的写 `ERR`
//! * `<名字>_pkt` — 帧级解析: `STP` / `SEQ 1` / `MWr32 len=1 tag=3 addr=0xF0001000` / `DATA DEADBEEF`
//!   / `LCRC ok` / `END` / `DLLP Ack seq=1` / `COM` `SKP` 等; 逻辑空闲 (D0.0) 显示空
//!
//! 处理流程: 从第一个边沿起按 UI 采样并在每个边沿重新对齐相位 (时钟恢复) -> 搜索 K28.5 comma
//! 做符号对齐 -> 8b/10b 查表 -> STP/SDP/END/COM 分帧 -> 解析 TLP 头与 DLLP。
//! LCRC / CRC16 按 PCIe Base Spec 的公式 (04C11DB7 / 100B, 取反, 位序反转) 校验并标出 ok / BAD。

use super::{BitTrace, Segment};
use std::collections::HashMap;

// ---------------------------------------------------------------- 8b/10b 表 (与 tools/gen_pcie_vcd.py 一致)
const D5B6B: [(&str, &str); 32] = [
    ("100111", "011000"), ("011101", "100010"), ("101101", "010010"), ("110001", "110001"),
    ("110101", "001010"), ("101001", "101001"), ("011001", "011001"), ("111000", "000111"),
    ("111001", "000110"), ("100101", "100101"), ("010101", "010101"), ("110100", "110100"),
    ("001101", "001101"), ("101100", "101100"), ("011100", "011100"), ("010111", "101000"),
    ("011011", "100100"), ("100011", "100011"), ("010011", "010011"), ("110010", "110010"),
    ("001011", "001011"), ("101010", "101010"), ("011010", "011010"), ("111010", "000101"),
    ("110011", "001100"), ("100110", "100110"), ("010110", "010110"), ("110110", "001001"),
    ("001110", "001110"), ("101110", "010001"), ("011110", "100001"), ("101011", "010100"),
];
const D3B4B: [(&str, &str); 8] = [
    ("1011", "0100"), ("1001", "1001"), ("0101", "0101"), ("1100", "0011"),
    ("1101", "0010"), ("1010", "1010"), ("0110", "0110"), ("1110", "0001"),
];
const D3B4B_ALT7: (&str, &str) = ("0111", "1000");
const K5B6B: [(u8, (&str, &str)); 5] = [
    (28, ("001111", "110000")), (23, ("111010", "000101")), (27, ("110110", "001001")),
    (29, ("101110", "010001")), (30, ("011110", "100001")),
];
const K3B4B: [(&str, &str); 8] = [
    ("1011", "0100"), ("0110", "1001"), ("1010", "0101"), ("1100", "0011"),
    ("1101", "0010"), ("0101", "1010"), ("1001", "0110"), ("0111", "1000"),
];

fn disparity(bits: &str) -> i32 {
    bits.chars().map(|c| if c == '1' { 1 } else { -1 }).sum()
}

/// 编码一个符号 (用于建解码表)。返回 10 位字符串和新的 RD。
fn encode(byte: u8, k: bool, rd: i32) -> Option<(String, i32)> {
    let (x, y) = ((byte & 0x1F) as usize, (byte >> 5) as usize);
    let six = if k {
        K5B6B.iter().find(|(kx, _)| *kx as usize == x).map(|(_, p)| if rd < 0 { p.0 } else { p.1 })?
    } else if rd < 0 {
        D5B6B[x].0
    } else {
        D5B6B[x].1
    };
    let mut rd2 = rd;
    if disparity(six) != 0 {
        rd2 += disparity(six);
    }
    let four = if k {
        if rd2 < 0 { K3B4B[y].0 } else { K3B4B[y].1 }
    } else if y == 7 && ((rd2 < 0 && [17, 18, 20].contains(&x)) || (rd2 > 0 && [11, 13, 14].contains(&x))) {
        if rd2 < 0 { D3B4B_ALT7.0 } else { D3B4B_ALT7.1 }
    } else if rd2 < 0 {
        D3B4B[y].0
    } else {
        D3B4B[y].1
    };
    let mut rd3 = rd2;
    if disparity(four) != 0 {
        rd3 += disparity(four);
    }
    Some((format!("{six}{four}"), rd3))
}

/// 10 位字符串 -> (字节, 是否 K 码)
fn build_decode_table() -> HashMap<String, (u8, bool)> {
    let mut m = HashMap::new();
    for rd in [-1, 1] {
        for b in 0..=255u8 {
            if let Some((code, _)) = encode(b, false, rd) {
                m.insert(code, (b, false));
            }
        }
        for (x, _) in K5B6B {
            // 合法的控制码只有 K28.0..K28.7 和 K23.7 / K27.7 / K29.7 / K30.7
            let ys: &[u8] = if x == 28 { &[0, 1, 2, 3, 4, 5, 6, 7] } else { &[7] };
            for &y in ys {
                let b = (y << 5) | x;
                if let Some((code, _)) = encode(b, true, rd) {
                    m.insert(code, (b, true));
                }
            }
        }
    }
    m
}

const K_COM: u8 = 0xBC; // K28.5
const K_SKP: u8 = 0x1C; // K28.0
const K_STP: u8 = 0xFB; // K27.7
const K_SDP: u8 = 0x5C; // K28.2
const K_END: u8 = 0xFD; // K29.7
const K_EDB: u8 = 0xFE; // K30.7
const K_PAD: u8 = 0xF7; // K23.7
const K_FTS: u8 = 0x3C; // K28.1
const K_IDL: u8 = 0x7C; // K28.3

fn k_name(b: u8) -> &'static str {
    match b {
        K_COM => "COM",
        K_SKP => "SKP",
        K_STP => "STP",
        K_SDP => "SDP",
        K_END => "END",
        K_EDB => "EDB",
        K_PAD => "PAD",
        K_FTS => "FTS",
        K_IDL => "IDL",
        _ => "K?",
    }
}

// ---------------------------------------------------------------- CRC
fn crc_generic(data: &[u8], poly: u64, width: u32, init: u64) -> u64 {
    let mask = if width == 64 { u64::MAX } else { (1u64 << width) - 1 };
    let mut crc = init;
    for &b in data {
        for i in (0..8).rev() {
            let bit = u64::from((b >> i) & 1);
            let fb = ((crc >> (width - 1)) & 1) ^ bit;
            crc = (crc << 1) & mask;
            if fb == 1 {
                crc ^= poly;
            }
        }
    }
    crc & mask
}

fn reflect(v: u64, width: u32) -> u64 {
    let mut r = 0;
    for i in 0..width {
        if v & (1 << i) != 0 {
            r |= 1 << (width - 1 - i);
        }
    }
    r
}

fn lcrc32(data: &[u8]) -> u32 {
    let c = crc_generic(data, 0x04C1_1DB7, 32, 0xFFFF_FFFF);
    reflect(!c & 0xFFFF_FFFF, 32) as u32
}

fn dllp_crc16(data: &[u8]) -> u16 {
    let c = crc_generic(data, 0x100B, 16, 0xFFFF);
    reflect(!c & 0xFFFF, 16) as u16
}

// ---------------------------------------------------------------- 比特恢复与符号对齐
#[derive(Debug, Clone, Copy)]
pub struct PcieConfig {
    /// 一个 UI 占多少个波形时间单位
    pub ui: f64,
    /// 是否解扰 (Gen1/Gen2 真实链路总是加扰的)
    pub descramble: bool,
}

/// Gen1/Gen2 扰码 LFSR: X^16+X^5+X^4+X^3+1, Galois 左移, 反馈掩码 0x39,
/// 输出取 D15 (移位前), 字节 LSB 先。复位后对 D0.0 输出 FF 17 C0 14 B2 E7 02 82 ...
struct Scrambler {
    lfsr: u16,
    in_ts: u8,
}

impl Scrambler {
    fn new() -> Self {
        Self { lfsr: 0xFFFF, in_ts: 0 }
    }

    fn advance_byte(&mut self) -> u8 {
        let mut out = 0u8;
        for i in 0..8 {
            let msb = (self.lfsr >> 15) & 1;
            out |= (msb as u8) << i;
            self.lfsr <<= 1;
            if msb == 1 {
                self.lfsr ^= 0x39;
            }
        }
        out
    }

    /// 处理一个符号, 返回解扰后的字节
    fn process(&mut self, byte: u8, k: bool, next_is_d: bool) -> u8 {
        if k {
            if byte == K_COM {
                self.lfsr = 0xFFFF;
                self.in_ts = if next_is_d { 15 } else { 0 };
                return byte;
            }
            if byte == K_SKP {
                return byte;
            }
            self.advance_byte();
            return byte;
        }
        let mask = self.advance_byte();
        if self.in_ts > 0 {
            self.in_ts -= 1;
            return byte;
        }
        byte ^ mask
    }
}

fn level_at(trace: &BitTrace, t: f64) -> bool {
    let idx = trace.partition_point(|&(tt, _)| (tt as f64) <= t);
    if idx == 0 { trace.first().map(|e| e.1).unwrap_or(false) } else { trace[idx - 1].1 }
}

/// 从边沿恢复时钟, 返回 (每位起始时间, 电平)
fn recover_bits(trace: &BitTrace, ui: f64) -> Vec<(f64, bool)> {
    let mut bits = vec![];
    if trace.len() < 2 {
        return bits;
    }
    // 第一个真正的跳变作为相位基准
    let first_edge = trace.windows(2).find(|w| w[0].1 != w[1].1).map(|w| w[1].0 as f64);
    let Some(mut cur) = first_edge else { return bits };
    // 把相位外推回波形起点, 这样第一个边沿之前的符号也能采到 (扰码 LFSR 从起点开始计数)
    let t0 = trace.first().map(|e| e.0 as f64).unwrap_or(0.0);
    while cur - ui >= t0 {
        cur -= ui;
    }
    let end = trace.last().map(|e| e.0 as f64).unwrap_or(cur) + ui * 12.0;
    let mut ei = trace.partition_point(|&(t, _)| (t as f64) < cur);
    while cur < end {
        bits.push((cur, level_at(trace, cur + ui * 0.5)));
        cur += ui;
        // 若下一位边界附近有跳变, 用跳变时间重新对齐相位
        while ei < trace.len() && (trace[ei].0 as f64) < cur - ui * 0.5 {
            ei += 1;
        }
        if ei < trace.len() {
            let te = trace[ei].0 as f64;
            if (te - cur).abs() <= ui * 0.5 {
                cur = te;
            }
        }
    }
    bits
}

/// 找 comma (K28.5) 确定符号边界, 返回偏移 0..10
fn find_alignment(bits: &[(f64, bool)]) -> Option<usize> {
    let s: String = bits.iter().map(|b| if b.1 { '1' } else { '0' }).collect();
    for pat in ["0011111010", "1100000101"] {
        if let Some(pos) = s.find(pat) {
            return Some(pos % 10);
        }
    }
    None
}

// ---------------------------------------------------------------- 帧解析
fn tlp_type_name(fmt: u8, typ: u8) -> String {
    let with_data = fmt & 2 != 0;
    let four_dw = fmt & 1 != 0;
    let base = match typ {
        0x00 => if with_data { "MWr" } else { "MRd" },
        0x01 => "MRdLk",
        0x02 => if with_data { "IOWr" } else { "IORd" },
        0x04 => if with_data { "CfgWr0" } else { "CfgRd0" },
        0x05 => if with_data { "CfgWr1" } else { "CfgRd1" },
        0x0A => if with_data { "CplD" } else { "Cpl" },
        0x0B => if with_data { "CplDLk" } else { "CplLk" },
        0x10..=0x17 => if with_data { "MsgD" } else { "Msg" },
        _ => "TLP?",
    };
    if typ == 0x00 || typ == 0x01 {
        format!("{base}{}", if four_dw { "64" } else { "32" })
    } else {
        base.to_string()
    }
}

fn describe_tlp(h: &[u8]) -> (String, usize) {
    if h.len() < 12 {
        return ("TLP hdr short".into(), h.len());
    }
    let fmt = h[0] >> 5;
    let typ = h[0] & 0x1F;
    let four_dw = fmt & 1 != 0;
    let hdr_len = if four_dw { 16 } else { 12 };
    let length = ((u16::from(h[2] & 0x03) << 8) | u16::from(h[3])) as u32;
    let length = if length == 0 { 1024 } else { length };
    let name = tlp_type_name(fmt, typ);
    let tc = (h[1] >> 4) & 7;
    let text = match typ {
        0x00..=0x02 => {
            let req = (u16::from(h[4]) << 8) | u16::from(h[5]);
            let tag = h[6];
            let be = h[7];
            let addr = if four_dw && h.len() >= 16 {
                (u64::from(h[8]) << 56) | (u64::from(h[9]) << 48) | (u64::from(h[10]) << 40) | (u64::from(h[11]) << 32)
                    | (u64::from(h[12]) << 24) | (u64::from(h[13]) << 16) | (u64::from(h[14]) << 8) | u64::from(h[15] & 0xFC)
            } else {
                (u64::from(h[8]) << 24) | (u64::from(h[9]) << 16) | (u64::from(h[10]) << 8) | u64::from(h[11] & 0xFC)
            };
            format!(
                "{name} len={length}DW req={:02X}:{:02X}.{} tag={tag} BE={:X}/{:X} addr=0x{addr:X}",
                req >> 8, (req >> 3) & 0x1F, req & 7, be >> 4, be & 0xF
            )
        }
        0x04 | 0x05 => {
            let req = (u16::from(h[4]) << 8) | u16::from(h[5]);
            let tag = h[6];
            let bdf = (u16::from(h[8]) << 8) | u16::from(h[9]);
            let reg = ((u16::from(h[10] & 0x0F) << 8) | u16::from(h[11])) & 0xFFC;
            format!(
                "{name} req={:04X} tag={tag} target={:02X}:{:02X}.{} reg=0x{reg:03X}",
                req, bdf >> 8, (bdf >> 3) & 0x1F, bdf & 7
            )
        }
        0x0A | 0x0B => {
            let cpl = (u16::from(h[4]) << 8) | u16::from(h[5]);
            let status = (h[6] >> 5) & 7;
            let bc = ((u16::from(h[6] & 0x0F) << 8) | u16::from(h[7])) as u32;
            let req = (u16::from(h[8]) << 8) | u16::from(h[9]);
            let tag = h[10];
            let st = match status { 0 => "SC", 1 => "UR", 2 => "CRS", 4 => "CA", _ => "?" };
            format!("{name} {st} cpl={cpl:04X} req={req:04X} tag={tag} bytes={bc} len={length}DW")
        }
        0x10..=0x17 => {
            let code = h[7];
            format!("{name} code=0x{code:02X} route={} tc={tc}", typ & 7)
        }
        _ => format!("{name} fmt={fmt} type=0x{typ:02X} len={length}DW"),
    };
    (text, hdr_len)
}

fn describe_dllp(p: &[u8]) -> String {
    if p.len() < 4 {
        return "DLLP short".into();
    }
    let t = p[0];
    let seq = (u16::from(p[2] & 0x0F) << 8) | u16::from(p[3]);
    match t {
        0x00 => format!("DLLP Ack seq={seq}"),
        0x10 => format!("DLLP Nak seq={seq}"),
        0x20 => "DLLP PM_Enter_L1".into(),
        0x21 => "DLLP PM_Enter_L23".into(),
        0x23 => "DLLP PM_Active_State_Request_L1".into(),
        0x24 => "DLLP PM_Request_Ack".into(),
        0x30 => "DLLP Vendor".into(),
        0x40..=0xFF => {
            let kind = match t >> 4 {
                0x4 => "InitFC1-P", 0x5 => "InitFC1-NP", 0x6 => "InitFC1-Cpl",
                0xC => "InitFC2-P", 0xD => "InitFC2-NP", 0xE => "InitFC2-Cpl",
                0x8 => "UpdateFC-P", 0x9 => "UpdateFC-NP", 0xA => "UpdateFC-Cpl",
                _ => "FC?",
            };
            let vc = t & 7;
            let hdr_fc = (u16::from(p[1] & 0x3F) << 2) | u16::from(p[2] >> 6);
            let data_fc = (u16::from(p[2] & 0x0F) << 8) | u16::from(p[3]);
            format!("DLLP {kind} vc={vc} hdr={hdr_fc} data={data_fc}")
        }
        _ => format!("DLLP type=0x{t:02X}"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Frame {
    Idle,
    Os,
    Tlp,
    Dllp,
}

/// 解码。返回 (符号信号, 帧信号)
pub fn decode(lane: &BitTrace, cfg: &PcieConfig) -> (Vec<Segment>, Vec<Segment>) {
    let table = build_decode_table();
    let mut sym_out = vec![Segment { time: 0, text: String::new() }];
    let mut pkt_out = vec![Segment { time: 0, text: String::new() }];

    let bits = recover_bits(lane, cfg.ui);
    let Some(offset) = find_alignment(&bits) else {
        sym_out.push(Segment { time: 0, text: "no comma (K28.5) found".into() });
        return (sym_out, pkt_out);
    };

    // 先查表得到线上符号 (时间, 字节, 是K, 有效)
    let mut raw: Vec<(u64, u8, bool, bool)> = vec![];
    let mut i = offset;
    while i + 10 <= bits.len() {
        let code: String = bits[i..i + 10].iter().map(|b| if b.1 { '1' } else { '0' }).collect();
        let t = bits[i].0.round() as u64;
        match table.get(&code) {
            Some(&(b, k)) => raw.push((t, b, k, true)),
            None => raw.push((t, 0, false, false)),
        }
        i += 10;
    }
    // 解扰, 生成符号信号
    let mut scr = Scrambler::new();
    let mut symbols: Vec<(u64, u8, bool, bool)> = Vec::with_capacity(raw.len());
    for (idx, &(t, b, k, ok)) in raw.iter().enumerate() {
        if !ok {
            sym_out.push(Segment { time: t, text: "ERR".into() });
            symbols.push((t, 0, false, false));
            scr = Scrambler::new();
            continue;
        }
        let name = format!("{}{}.{}", if k { 'K' } else { 'D' }, b & 0x1F, b >> 5);
        let next_is_d = raw.get(idx + 1).is_some_and(|n| n.3 && !n.2);
        let data = if cfg.descramble { scr.process(b, k, next_is_d) } else { b };
        let text = if k {
            format!("{name} {}", k_name(b))
        } else if data != b {
            format!("{name} 0x{b:02X}>0x{data:02X}")
        } else {
            format!("{name} 0x{b:02X}")
        };
        sym_out.push(Segment { time: t, text });
        symbols.push((t, data, k, true));
    }
    if let Some(last) = bits.last() {
        sym_out.push(Segment { time: (last.0 + cfg.ui).round() as u64, text: String::new() });
    }

    // 帧解析
    let mut frame = Frame::Idle;
    let mut buf: Vec<(u64, u8)> = vec![];
    let mut idle_pending = false;
    for &(t, b, k, ok) in &symbols {
        if !ok {
            pkt_out.push(Segment { time: t, text: "ERR".into() });
            frame = Frame::Idle;
            buf.clear();
            continue;
        }
        match frame {
            Frame::Idle | Frame::Os => {
                if k {
                    match b {
                        K_STP => {
                            frame = Frame::Tlp;
                            buf.clear();
                            pkt_out.push(Segment { time: t, text: "STP".into() });
                        }
                        K_SDP => {
                            frame = Frame::Dllp;
                            buf.clear();
                            pkt_out.push(Segment { time: t, text: "SDP".into() });
                        }
                        _ => {
                            frame = Frame::Os;
                            pkt_out.push(Segment { time: t, text: k_name(b).into() });
                        }
                    }
                    idle_pending = false;
                } else if b == 0x00 {
                    if frame == Frame::Os || !idle_pending {
                        pkt_out.push(Segment { time: t, text: String::new() });
                        idle_pending = true;
                    }
                    frame = Frame::Idle;
                } else {
                    // 有序集里的数据符号 (如 TS1/TS2 的 D 码)
                    pkt_out.push(Segment { time: t, text: format!("D 0x{b:02X}") });
                    frame = Frame::Os;
                }
            }
            Frame::Tlp | Frame::Dllp => {
                if k && (b == K_END || b == K_EDB) {
                    if frame == Frame::Tlp {
                        finish_tlp(&buf, &mut pkt_out);
                    } else {
                        finish_dllp(&buf, &mut pkt_out);
                    }
                    pkt_out.push(Segment { time: t, text: if b == K_END { "END".into() } else { "EDB (nullified)".into() } });
                    frame = Frame::Idle;
                    idle_pending = false;
                    buf.clear();
                } else if k {
                    pkt_out.push(Segment { time: t, text: format!("unexpected {}", k_name(b)) });
                    frame = Frame::Idle;
                    buf.clear();
                } else {
                    buf.push((t, b));
                }
            }
        }
    }
    pkt_out.sort_by_key(|s| s.time);
    (sym_out, pkt_out)
}

fn finish_tlp(buf: &[(u64, u8)], out: &mut Vec<Segment>) {
    if buf.len() < 2 + 12 + 4 {
        if let Some(f) = buf.first() {
            out.push(Segment { time: f.0, text: format!("TLP too short ({} bytes)", buf.len()) });
        }
        return;
    }
    let bytes: Vec<u8> = buf.iter().map(|b| b.1).collect();
    let seq = (u16::from(bytes[0] & 0x0F) << 8) | u16::from(bytes[1]);
    out.push(Segment { time: buf[0].0, text: format!("SEQ {seq}") });
    let body = &bytes[2..bytes.len() - 4];
    let (text, hdr_len) = describe_tlp(body);
    out.push(Segment { time: buf[2].0, text });
    let data = &body[hdr_len.min(body.len())..];
    if !data.is_empty() {
        let hex: String = data.iter().map(|b| format!("{b:02X}")).collect();
        let shown = if hex.len() > 32 { format!("{}…", &hex[..32]) } else { hex };
        out.push(Segment { time: buf[2 + hdr_len].0, text: format!("DATA {shown}") });
    }
    let crc_pos = bytes.len() - 4;
    let got = (u32::from(bytes[crc_pos]) << 24) | (u32::from(bytes[crc_pos + 1]) << 16)
        | (u32::from(bytes[crc_pos + 2]) << 8) | u32::from(bytes[crc_pos + 3]);
    let want = lcrc32(&bytes[..crc_pos]);
    out.push(Segment {
        time: buf[crc_pos].0,
        text: if got == want { format!("LCRC 0x{got:08X} ok") } else { format!("LCRC 0x{got:08X} BAD (calc 0x{want:08X})") },
    });
}

fn finish_dllp(buf: &[(u64, u8)], out: &mut Vec<Segment>) {
    if buf.len() < 6 {
        if let Some(f) = buf.first() {
            out.push(Segment { time: f.0, text: format!("DLLP too short ({} bytes)", buf.len()) });
        }
        return;
    }
    let bytes: Vec<u8> = buf.iter().map(|b| b.1).collect();
    out.push(Segment { time: buf[0].0, text: describe_dllp(&bytes[..4]) });
    let got = (u16::from(bytes[4]) << 8) | u16::from(bytes[5]);
    let want = dllp_crc16(&bytes[..4]);
    out.push(Segment {
        time: buf[4].0,
        text: if got == want { format!("CRC16 0x{got:04X} ok") } else { format!("CRC16 0x{got:04X} BAD (calc 0x{want:04X})") },
    });
}

/// 参数: gen1 / gen2 / ui=<ps>
pub fn config_from_params(params: &[String], units_per_second: f64) -> Result<PcieConfig, String> {
    let mut rate = 2.5e9;
    let mut ui_ps: Option<f64> = None;
    let mut descramble = true;
    for p in params {
        let lp = p.to_ascii_lowercase();
        match lp.as_str() {
            "gen1" => rate = 2.5e9,
            "gen2" => rate = 5.0e9,
            "noscramble" | "raw" => descramble = false,
            _ => {
                if let Some(v) = lp.strip_prefix("ui=").and_then(|s| s.trim_end_matches("ps").parse::<f64>().ok()) {
                    ui_ps = Some(v);
                } else {
                    return Err(format!("无法识别的 PCIe 参数 '{p}' (示例: gen1, gen2, ui=400ps, noscramble)"));
                }
            }
        }
    }
    let ui_s = ui_ps.map_or(1.0 / rate, |ps| ps * 1e-12);
    let ui = ui_s * units_per_second;
    if ui < 2.0 {
        return Err(format!("波形时基太粗: 一个 UI 只有 {ui:.2} 个时间单位, 至少需要 2"));
    }
    Ok(PcieConfig { ui, descramble })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_roundtrip_and_comma() {
        let t = build_decode_table();
        assert_eq!(t.get("0011111010"), Some(&(0xBC, true)));
        assert_eq!(t.get("1100000101"), Some(&(0xBC, true)));
        let (c, _) = encode(0x00, false, -1).unwrap();
        assert_eq!(t.get(&c), Some(&(0x00, false)));
    }

    #[test]
    fn scrambler_matches_spec_sequence() {
        let mut s = Scrambler::new();
        let seq: Vec<u8> = (0..8).map(|_| s.process(0x00, false, false)).collect();
        assert_eq!(seq, vec![0xFF, 0x17, 0xC0, 0x14, 0xB2, 0xE7, 0x02, 0x82]);
    }

    #[test]
    fn crc_known_values() {
        // 与 tools/gen_pcie_vcd.py 相同算法: 自洽即可
        let d = [0x00, 0x00, 0x00, 0x01];
        assert_eq!(dllp_crc16(&d), dllp_crc16(&d));
        assert_ne!(lcrc32(&[1, 2, 3]), lcrc32(&[1, 2, 4]));
    }
}

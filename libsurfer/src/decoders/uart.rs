//! UART (异步串口) 解码器。
//!
//! 输入: 一根数据线 (RX 或 TX) 的电平变化。参数: 波特率、数据位数、校验、停止位。
//! 输出: 每个字节一个段, 从起始位下降沿开始, 文本形如 `0x4D 'M'`;
//! 帧错误 (停止位不为高) 标成 `0x4D FRAME?`, 校验错误标成 `PARITY?`。
//! 空闲期间显示空字符串。

use super::{BitTrace, Segment};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Parity {
    None,
    Even,
    Odd,
}

#[derive(Debug, Clone, Copy)]
pub struct UartConfig {
    pub baud: f64,
    pub data_bits: u8,
    pub parity: Parity,
    pub stop_bits: u8,
    /// 时间单位换算: 1 秒等于多少个波形时间单位 (VCD 时基 1ns 时为 1e9)
    pub units_per_second: f64,
    /// 线路电平是否反相 (TTL 串口正常为 空闲=高)
    pub inverted: bool,
}

impl Default for UartConfig {
    fn default() -> Self {
        Self {
            baud: 115_200.0,
            data_bits: 8,
            parity: Parity::None,
            stop_bits: 1,
            units_per_second: 1e9,
            inverted: false,
        }
    }
}

/// 在时刻 t 读取线路电平 (trace 按时间排序, 取最后一次 <= t 的变化)
fn level_at(trace: &BitTrace, t: f64, idle: bool) -> bool {
    let idx = trace.partition_point(|&(tt, _)| (tt as f64) <= t);
    if idx == 0 { idle } else { trace[idx - 1].1 }
}

fn printable(b: u32) -> String {
    match char::from_u32(b) {
        Some(c) if (0x20..0x7f).contains(&b) => format!("'{c}'"),
        _ => match b {
            0x0A => "LF".into(),
            0x0D => "CR".into(),
            0x09 => "TAB".into(),
            0x00 => "NUL".into(),
            _ => String::new(),
        },
    }
}

pub fn decode(line: &BitTrace, cfg: &UartConfig) -> Vec<Segment> {
    let bit_time = cfg.units_per_second / cfg.baud;
    let idle_level = !cfg.inverted; // 空闲电平
    let mut out = vec![Segment {
        time: 0,
        text: String::new(),
    }];
    // 逐个起始位: 找 "变为非空闲电平" 的边沿
    let mut i = 0usize;
    let mut resume_time = 0.0f64;
    while i < line.len() {
        let (t, lvl) = line[i];
        let t = t as f64;
        i += 1;
        if lvl == idle_level || t < resume_time {
            continue;
        }
        // 起始位: 在 1/2 位处确认仍为非空闲, 否则是毛刺
        let mid = t + bit_time * 0.5;
        if level_at(line, mid, idle_level) == idle_level {
            continue;
        }
        // 采样数据位 (LSB 先)
        let mut value: u32 = 0;
        let mut ones = 0u32;
        for k in 0..cfg.data_bits {
            let ts = t + bit_time * (1.5 + k as f64);
            let bit = level_at(line, ts, idle_level) ^ cfg.inverted;
            if bit {
                value |= 1 << k;
                ones += 1;
            }
        }
        let mut pos = 1.0 + cfg.data_bits as f64;
        let mut errors = vec![];
        if cfg.parity != Parity::None {
            let ts = t + bit_time * (pos + 0.5);
            let pbit = level_at(line, ts, idle_level) ^ cfg.inverted;
            if pbit {
                ones += 1;
            }
            let ok = match cfg.parity {
                Parity::Even => ones % 2 == 0,
                Parity::Odd => ones % 2 == 1,
                Parity::None => true,
            };
            if !ok {
                errors.push("PARITY?");
            }
            pos += 1.0;
        }
        // 停止位
        let ts = t + bit_time * (pos + 0.5);
        // 停止位应为空闲电平
        if !(level_at(line, ts, idle_level) ^ cfg.inverted) {
            errors.push("FRAME?");
        }
        let end = t + bit_time * (pos + cfg.stop_bits as f64);
        let mut text = format!("0x{value:02X}");
        let p = printable(value);
        if !p.is_empty() {
            text.push(' ');
            text.push_str(&p);
        }
        for e in errors {
            text.push(' ');
            text.push_str(e);
        }
        out.push(Segment {
            time: t.round() as u64,
            text,
        });
        out.push(Segment {
            time: end.round() as u64,
            text: String::new(),
        });
        resume_time = end - bit_time * 0.25; // 允许下一帧的起始位紧贴停止位
    }
    out
}

/// 解析命令行参数: `[baud] [8N1]`。示例: `115200`, `9600 8E1`。
pub fn config_from_params(params: &[String], units_per_second: f64) -> Result<UartConfig, String> {
    let mut cfg = UartConfig {
        units_per_second,
        ..Default::default()
    };
    for p in params {
        let up = p.to_ascii_uppercase();
        if up == "INV" || up == "INVERTED" {
            cfg.inverted = true;
            continue;
        }
        // 8N1 / 7E2 ... (要先于数字判断: "8E1" 会被当成科学计数法的 80)
        let chars: Vec<char> = up.chars().collect();
        if chars.len() == 3 && chars[0].is_ascii_digit() && chars[2].is_ascii_digit() {
            cfg.data_bits = chars[0].to_digit(10).unwrap() as u8;
            cfg.parity = match chars[1] {
                'N' => Parity::None,
                'E' => Parity::Even,
                'O' => Parity::Odd,
                other => return Err(format!("未知校验方式 '{other}', 应为 N/E/O")),
            };
            cfg.stop_bits = chars[2].to_digit(10).unwrap() as u8;
            continue;
        }
        if let Ok(b) = p.parse::<f64>() {
            cfg.baud = b;
            continue;
        }
        return Err(format!("无法识别的 UART 参数 '{p}' (示例: 115200 8N1 inv)"));
    }
    if cfg.baud <= 0.0 || !(5..=9).contains(&cfg.data_bits) {
        return Err("UART 参数超出范围".into());
    }
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(trace: &mut BitTrace, t0: u64, bit: u64, byte: u8) -> u64 {
        trace.push((t0, false));
        for k in 0..8 {
            trace.push((t0 + bit * (k + 1), (byte >> k) & 1 == 1));
        }
        trace.push((t0 + bit * 9, true));
        t0 + bit * 10
    }

    #[test]
    fn decodes_two_bytes() {
        let bit = 8681; // 115200 baud @1ns
        let mut tr: BitTrace = vec![(0, true)];
        let t = frame(&mut tr, 1000, bit, b'M');
        frame(&mut tr, t + 5000, bit, 0x0A);
        let cfg = UartConfig::default();
        let texts: Vec<String> = decode(&tr, &cfg)
            .into_iter()
            .map(|s| s.text)
            .filter(|s| !s.is_empty())
            .collect();
        assert_eq!(texts, vec!["0x4D 'M'", "0x0A LF"]);
    }
}

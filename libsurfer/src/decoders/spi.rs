//! SPI 解码器。
//!
//! 输入顺序: `SCLK, MOSI, [MISO], [CS]` (不存在的用空 trace 占位)。
//! 参数: `mode 0..3` (CPOL/CPHA), `bits N` (默认 8), `lsb` (LSB 先, 默认 MSB 先),
//! `cs_high` (片选高有效, 默认低有效)。
//! 输出: 每个字一个段, 文本形如 `M:0xA5 S:0x3C` (只有 MOSI 时为 `0xA5`)。
//! 有 CS 时只在片选有效期间解码, 片选无效时清空位计数; 没有 CS 时连续按 N 位分组。

use super::{BitTrace, Segment};

#[derive(Debug, Clone, Copy)]
pub struct SpiConfig {
    pub cpol: bool,
    pub cpha: bool,
    pub bits: u8,
    pub msb_first: bool,
    pub cs_active_high: bool,
}

impl Default for SpiConfig {
    fn default() -> Self {
        Self {
            cpol: false,
            cpha: false,
            bits: 8,
            msb_first: true,
            cs_active_high: false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Line {
    Sclk,
    Mosi,
    Miso,
    Cs,
}

pub fn decode(sclk: &BitTrace, mosi: &BitTrace, miso: &BitTrace, cs: &BitTrace, cfg: &SpiConfig) -> Vec<Segment> {
    let has_miso = !miso.is_empty();
    let has_cs = !cs.is_empty();

    let mut events: Vec<(u64, Line, bool)> = Vec::new();
    events.extend(sclk.iter().map(|&(t, v)| (t, Line::Sclk, v)));
    events.extend(mosi.iter().map(|&(t, v)| (t, Line::Mosi, v)));
    events.extend(miso.iter().map(|&(t, v)| (t, Line::Miso, v)));
    events.extend(cs.iter().map(|&(t, v)| (t, Line::Cs, v)));
    // 同一时刻: 先处理数据线, 再处理时钟 (数据在采样沿前已稳定)
    events.sort_by_key(|e| (e.0, matches!(e.1, Line::Sclk)));

    // 采样沿: CPHA=0 在第一个沿 (相对空闲电平的跳变) 采样, CPHA=1 在第二个沿
    // 空闲电平 = CPOL。第一个沿 = 从 CPOL 跳到 !CPOL。
    let sample_on_rising = cfg.cpol == cfg.cpha; // mode0: rising, mode1: falling, mode2: falling, mode3: rising

    let mut sclk_lvl = cfg.cpol;
    let mut mosi_lvl = false;
    let mut miso_lvl = false;
    let mut cs_active = !has_cs; // 无 CS 视为一直有效
    let mut nbits = 0u8;
    let mut mo: u64 = 0;
    let mut mi: u64 = 0;
    let mut word_start: u64 = 0;
    let mut out = vec![Segment {
        time: 0,
        text: String::new(),
    }];

    for (t, line, lvl) in events {
        match line {
            Line::Mosi => mosi_lvl = lvl,
            Line::Miso => miso_lvl = lvl,
            Line::Cs => {
                let active = lvl == cfg.cs_active_high;
                if active != cs_active {
                    cs_active = active;
                    nbits = 0;
                    mo = 0;
                    mi = 0;
                    if !active {
                        out.push(Segment {
                            time: t,
                            text: String::new(),
                        });
                    }
                }
            }
            Line::Sclk => {
                let rising = lvl && !sclk_lvl;
                let falling = !lvl && sclk_lvl;
                sclk_lvl = lvl;
                let sample = (rising && sample_on_rising) || (falling && !sample_on_rising);
                if !sample || !cs_active {
                    continue;
                }
                if nbits == 0 {
                    word_start = t;
                }
                if cfg.msb_first {
                    mo = (mo << 1) | u64::from(mosi_lvl);
                    mi = (mi << 1) | u64::from(miso_lvl);
                } else {
                    mo |= u64::from(mosi_lvl) << nbits;
                    mi |= u64::from(miso_lvl) << nbits;
                }
                nbits += 1;
                if nbits == cfg.bits {
                    let w = usize::from(cfg.bits).div_ceil(4);
                    let text = if has_miso {
                        format!("M:0x{mo:0w$X} S:0x{mi:0w$X}")
                    } else {
                        format!("0x{mo:0w$X}")
                    };
                    out.push(Segment {
                        time: word_start,
                        text,
                    });
                    nbits = 0;
                    mo = 0;
                    mi = 0;
                }
            }
        }
    }
    out
}

/// 解析参数: `mode0..mode3` / `0..3`, `bits16`, `lsb`, `cs_high`
pub fn config_from_params(params: &[String]) -> Result<SpiConfig, String> {
    let mut cfg = SpiConfig::default();
    for p in params {
        let lp = p.to_ascii_lowercase();
        if let Some(m) = lp.strip_prefix("mode").or(Some(lp.as_str())).and_then(|s| s.parse::<u8>().ok()) {
            if m > 3 {
                return Err(format!("SPI 模式 {m} 无效, 应为 0..3"));
            }
            cfg.cpol = m & 2 != 0;
            cfg.cpha = m & 1 != 0;
            continue;
        }
        if let Some(b) = lp.strip_prefix("bits").and_then(|s| s.parse::<u8>().ok()) {
            if !(1..=64).contains(&b) {
                return Err("SPI 位宽应在 1..64".into());
            }
            cfg.bits = b;
            continue;
        }
        match lp.as_str() {
            "lsb" => cfg.msb_first = false,
            "msb" => cfg.msb_first = true,
            "cs_high" => cfg.cs_active_high = true,
            _ => return Err(format!("无法识别的 SPI 参数 '{p}' (示例: mode0 bits8 lsb cs_high)")),
        }
    }
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode0_two_bytes_with_cs() {
        let mut sclk: BitTrace = vec![(0, false)];
        let mut mosi: BitTrace = vec![(0, false)];
        let mut miso: BitTrace = vec![(0, false)];
        let mut cs: BitTrace = vec![(0, true)];
        let mut t = 100;
        cs.push((t, false));
        for (a, b) in [(0xA5u8, 0x3Cu8), (0x01, 0xFF)] {
            for k in (0..8).rev() {
                t += 10;
                mosi.push((t, (a >> k) & 1 == 1));
                miso.push((t, (b >> k) & 1 == 1));
                t += 10;
                sclk.push((t, true));
                t += 10;
                sclk.push((t, false));
            }
        }
        t += 10;
        cs.push((t, true));
        let segs = decode(&sclk, &mosi, &miso, &cs, &SpiConfig::default());
        let texts: Vec<String> = segs.into_iter().map(|s| s.text).filter(|s| !s.is_empty()).collect();
        assert_eq!(texts, vec!["M:0xA5 S:0x3C", "M:0x01 S:0xFF"]);
    }
}

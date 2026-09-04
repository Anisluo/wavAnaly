//! I2C bus decoder.
//!
//! Inputs: `scl`, `sda` bit traces. Output segments:
//! * `S` / `Sr` / `P` for START, repeated START and STOP conditions
//! * `0x36 W` / `0x36 R` for the 7-bit address byte plus direction
//! * `0x0C` for data bytes
//! * `ACK` / `NACK` for the ninth clock
//! * `` (empty) while the bus is idle
//!
//! Following the specification, SDA is sampled on the rising edge of SCL, and
//! a change of SDA while SCL is high is a START (falling) or STOP (rising).

use super::{BitTrace, Segment};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Edge {
    Scl,
    Sda,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// No START seen yet
    Idle,
    /// Inside a transaction; `first` is true until the address byte is done
    Active { first: bool },
}

pub fn decode(scl: &BitTrace, sda: &BitTrace) -> Vec<Segment> {
    // Merge both traces into one time-ordered edge stream.
    let mut events: Vec<(u64, Edge, bool)> = scl
        .iter()
        .map(|&(t, v)| (t, Edge::Scl, v))
        .chain(sda.iter().map(|&(t, v)| (t, Edge::Sda, v)))
        .collect();
    // Stable sort keeps SCL before SDA at equal timestamps, which matches the
    // usual "SDA changes right after SCL falls" ordering in dumps.
    events.sort_by_key(|e| e.0);

    let mut out = vec![Segment {
        time: 0,
        text: String::new(),
    }];
    let mut scl_level = scl.first().map(|e| e.1).unwrap_or(true);
    let mut sda_level = sda.first().map(|e| e.1).unwrap_or(true);
    let mut phase = Phase::Idle;
    let mut bits: Vec<bool> = Vec::with_capacity(8);
    let mut byte_start: Option<u64> = None;

    for (t, edge, level) in events {
        match edge {
            Edge::Sda => {
                let changed = level != sda_level;
                sda_level = level;
                if !changed || !scl_level {
                    continue;
                }
                // SDA changed while SCL high -> START or STOP condition
                if level {
                    out.push(Segment {
                        time: t,
                        text: "P".into(),
                    });
                    phase = Phase::Idle;
                } else {
                    let text = match phase {
                        Phase::Idle => "S",
                        Phase::Active { .. } => "Sr",
                    };
                    out.push(Segment {
                        time: t,
                        text: text.into(),
                    });
                    phase = Phase::Active { first: true };
                }
                bits.clear();
                byte_start = None;
            }
            Edge::Scl => {
                let rising = level && !scl_level;
                let falling = !level && scl_level;
                scl_level = level;
                if falling && phase == Phase::Idle {
                    // bus idle after STOP: clear the label once the clock moves on
                    if out.last().is_some_and(|s| s.text == "P") {
                        out.push(Segment {
                            time: t,
                            text: String::new(),
                        });
                    }
                    continue;
                }
                if !rising {
                    continue;
                }
                let Phase::Active { first } = phase else {
                    continue;
                };
                // Sample SDA on SCL rising edge
                if bits.len() == 8 {
                    // ninth clock: acknowledge bit
                    out.push(Segment {
                        time: t,
                        text: if sda_level { "NACK".into() } else { "ACK".into() },
                    });
                    bits.clear();
                    byte_start = None;
                    if first {
                        phase = Phase::Active { first: false };
                    }
                    continue;
                }
                if bits.is_empty() {
                    byte_start = Some(t);
                }
                bits.push(sda_level);
                if bits.len() == 8 {
                    let byte = bits
                        .iter()
                        .fold(0u8, |acc, &b| (acc << 1) | u8::from(b));
                    let text = if first {
                        format!(
                            "0x{:02X} {}",
                            byte >> 1,
                            if byte & 1 == 1 { "R" } else { "W" }
                        )
                    } else {
                        format!("0x{byte:02X}")
                    };
                    out.push(Segment {
                        time: byte_start.unwrap_or(t),
                        text,
                    });
                }
            }
        }
    }
    // After the last STOP the bus stays idle
    if phase == Phase::Idle && out.last().is_some_and(|s| s.text == "P") {
        let t = out.last().map(|s| s.time).unwrap_or(0) + 1;
        out.push(Segment {
            time: t,
            text: String::new(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn send_byte(byte: u8, t: &mut u64, scl: &mut BitTrace, sda: &mut BitTrace) {
        for k in (0..8).rev() {
            *t += 10;
            sda.push((*t, (byte >> k) & 1 == 1));
            *t += 10;
            scl.push((*t, true));
            *t += 10;
            scl.push((*t, false));
        }
        // ack
        *t += 10;
        sda.push((*t, false));
        *t += 10;
        scl.push((*t, true));
        *t += 10;
        scl.push((*t, false));
    }

    /// Build SCL/SDA traces for: S, 0x36 W, ACK, 0x0C, ACK, P
    fn build() -> (BitTrace, BitTrace) {
        let mut scl = vec![(0, true)];
        let mut sda = vec![(0, true)];
        let mut t = 100;
        sda.push((t, false)); // START
        t += 50;
        scl.push((t, false));
        send_byte(0x6C, &mut t, &mut scl, &mut sda);
        send_byte(0x0C, &mut t, &mut scl, &mut sda);
        t += 10;
        sda.push((t, false));
        t += 10;
        scl.push((t, true));
        t += 10;
        sda.push((t, true)); // STOP
        (scl, sda)
    }

    #[test]
    fn decodes_write_transaction() {
        let (scl, sda) = build();
        let texts: Vec<String> = decode(&scl, &sda)
            .into_iter()
            .map(|s| s.text)
            .filter(|s| !s.is_empty())
            .collect();
        assert_eq!(texts, vec!["S", "0x36 W", "ACK", "0x0C", "ACK", "P"]);
    }
}

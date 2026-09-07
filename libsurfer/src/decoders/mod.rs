//! Protocol decoders that turn several physical signals into one "virtual"
//! string-valued signal (e.g. `SCL` + `SDA` -> decoded I2C transactions).
//!
//! The decoded result is stored in the wave container as a [`VirtualSignal`]
//! and displayed like any other string variable, so all existing zoom, cursor
//! and marker tooling works on it without changes.

pub mod i2c;
pub mod spi;
pub mod uart;

use eyre::{Result, bail};
use num::{BigUint, ToPrimitive};
use surfer_translation_types::VariableValue;

use crate::wave_container::QueryResult;

/// One decoded segment: the value becomes visible at `time` and stays until the
/// next segment starts.
#[derive(Debug, Clone)]
pub struct Segment {
    pub time: u64,
    pub text: String,
}

/// A precomputed, string-valued signal that is not backed by the waveform file.
#[derive(Debug)]
pub struct VirtualSignal {
    pub type_name: String,
    /// Sorted by time, strictly increasing.
    pub segments: Vec<Segment>,
}

impl VirtualSignal {
    pub fn new(type_name: impl Into<String>, mut segments: Vec<Segment>) -> Self {
        segments.sort_by_key(|s| s.time);
        // collapse segments that share a timestamp: keep the last one
        segments.dedup_by(|b, a| {
            if a.time == b.time {
                a.text = std::mem::take(&mut b.text);
                true
            } else {
                false
            }
        });
        Self {
            type_name: type_name.into(),
            segments,
        }
    }

    /// Same semantics as `WellenContainer::query_variable`.
    pub fn query(&self, time: &BigUint) -> QueryResult {
        let Some(t) = time.to_u64() else {
            return QueryResult {
                current: None,
                next: None,
            };
        };
        // index of the first segment with segment.time > t
        let idx = self.segments.partition_point(|s| s.time <= t);
        let current = idx.checked_sub(1).map(|i| {
            let s = &self.segments[i];
            (
                BigUint::from(s.time),
                VariableValue::String(s.text.clone()),
            )
        });
        let next = self.segments.get(idx).map(|s| BigUint::from(s.time));
        QueryResult { current, next }
    }
}

/// A single-bit input, as a list of (time, level) changes.
pub type BitTrace = Vec<(u64, bool)>;

/// Convert raw variable changes to a bit trace. `x`/`z` are treated as high,
/// since open-drain buses are pulled up.
pub fn to_bit_trace(changes: impl Iterator<Item = (u64, VariableValue)>) -> BitTrace {
    changes
        .map(|(t, v)| {
            let level = match v {
                VariableValue::BigUint(b) => b != BigUint::from(0u8),
                VariableValue::String(s) => !s.ends_with('0'),
            };
            (t, level)
        })
        .collect()
}

/// Names of all available protocol decoders, for command completion.
pub const PROTOCOLS: &[&str] = &["i2c", "uart", "spi"];

/// Run the decoder called `protocol` on the given input traces. The order of
/// `inputs` is protocol specific (see the individual decoder modules).
///
/// `params` are the free-form decoder options typed after the signal names
/// (baud rate, SPI mode, ...). `units_per_second` converts the waveform's
/// time unit to seconds (1e9 for a 1 ns timescale).
pub fn run(
    protocol: &str,
    inputs: &[BitTrace],
    params: &[String],
    units_per_second: f64,
) -> Result<VirtualSignal> {
    match protocol {
        "i2c" => {
            if inputs.len() != 2 {
                bail!("i2c decoder needs exactly two inputs: SCL SDA");
            }
            Ok(VirtualSignal::new(
                "decoded i2c",
                i2c::decode(&inputs[0], &inputs[1]),
            ))
        }
        "uart" => {
            if inputs.len() != 1 {
                bail!("uart decoder needs exactly one input line (RX or TX)");
            }
            let cfg = uart::config_from_params(params, units_per_second).map_err(|e| eyre::eyre!(e))?;
            Ok(VirtualSignal::new(
                format!("decoded uart {} {}{}{}", cfg.baud, cfg.data_bits,
                    match cfg.parity { uart::Parity::None => "N", uart::Parity::Even => "E", uart::Parity::Odd => "O" },
                    cfg.stop_bits),
                uart::decode(&inputs[0], &cfg),
            ))
        }
        "spi" => {
            if inputs.len() < 2 || inputs.len() > 4 {
                bail!("spi decoder needs SCLK MOSI [MISO] [CS] (use '-' for a missing line)");
            }
            let cfg = spi::config_from_params(params).map_err(|e| eyre::eyre!(e))?;
            let empty = BitTrace::new();
            let miso = inputs.get(2).unwrap_or(&empty);
            let cs = inputs.get(3).unwrap_or(&empty);
            Ok(VirtualSignal::new(
                format!("decoded spi mode{}", u8::from(cfg.cpol) * 2 + u8::from(cfg.cpha)),
                spi::decode(&inputs[0], &inputs[1], miso, cs, &cfg),
            ))
        }
        other => bail!("Unknown protocol decoder '{other}'"),
    }
}

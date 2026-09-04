//! Protocol decoders that turn several physical signals into one "virtual"
//! string-valued signal (e.g. `SCL` + `SDA` -> decoded I2C transactions).
//!
//! The decoded result is stored in the wave container as a [`VirtualSignal`]
//! and displayed like any other string variable, so all existing zoom, cursor
//! and marker tooling works on it without changes.

pub mod i2c;

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
pub const PROTOCOLS: &[&str] = &["i2c"];

/// Run the decoder called `protocol` on the given input traces. The order of
/// `inputs` is protocol specific (see the individual decoder modules).
pub fn run(protocol: &str, inputs: &[BitTrace]) -> Result<VirtualSignal> {
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
        other => bail!("Unknown protocol decoder '{other}'"),
    }
}

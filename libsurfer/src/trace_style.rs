use derive_more::{Display, FromStr};
use enum_iterator::Sequence;
use num::Zero as _;
use serde::{Deserialize, Serialize};
use surfer_translation_types::VariableValue;

/// Selects the drawing style for digital waveform traces.
///
/// - `Default`: Standard trace drawing with both upper and lower lines for all values.
/// - `Dinotrace`: Dinotrace-inspired style. All-zero vectors have no upper line and a bold lower
///   line. All-one vectors have a bold upper line and no lower line.
/// - `Zero`: All-zero vectors are drawn without the upper line. Other vectors use standard drawing.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Display, FromStr, PartialEq, Eq, Sequence, Serialize,
)]
pub enum TraceStyle {
    #[default]
    Default,
    Dinotrace,
    Zero,
}

/// Information about values that enable special drawing of all-0 and all-1 values.
///
/// - `Normal`: Standard trace drawing applies.
/// - `AllZeros`: Value is all zeros; may be drawn with reduced upper line depending on style.
/// - `AllZerosThick`: Value is all zeros in Dinotrace style; drawn with bold lower line and no upper line.
/// - `AllOnes`: Value is all ones in Dinotrace style; drawn with bold upper line.
#[derive(Clone, Copy)]
pub(crate) enum TraceValue {
    Normal,
    AllZeros,
    AllZerosThick,
    AllOnes,
}

impl TraceValue {
    /// Determines the special trace value representation based on the signal value and style.
    ///
    /// For `TraceStyle::Default`, always returns `TraceValue::Normal`.
    ///
    /// For `TraceStyle::Dinotrace`:
    /// - All-zero values return `TraceValue::AllZerosThick`.
    /// - All-one values (all bits set) return `TraceValue::AllOnes`.
    /// - Other values return `TraceValue::Normal`.
    ///
    /// For `TraceStyle::Zero`:
    /// - All-zero values return `TraceValue::AllZeros`.
    /// - Other values return `TraceValue::Normal`.
    ///
    /// # Arguments
    ///
    /// * `val` - The signal value to analyze.
    /// * `num_bits` - The bit width of the signal. Required to determine if all bits are set.
    /// * `trace_style` - The trace drawing style to apply.
    pub(crate) fn from_value(
        val: &VariableValue,
        num_bits: Option<u32>,
        trace_style: TraceStyle,
    ) -> Self {
        if trace_style == TraceStyle::Default {
            return Self::Normal;
        }
        match val {
            VariableValue::BigUint(u) if u.is_zero() => {
                if trace_style == TraceStyle::Dinotrace {
                    TraceValue::AllZerosThick
                } else {
                    TraceValue::AllZeros
                }
            }
            VariableValue::BigUint(u)
                if trace_style == TraceStyle::Dinotrace
                    && num_bits.is_some_and(|bits| u.count_ones() == u64::from(bits)) =>
            {
                TraceValue::AllOnes
            }
            VariableValue::BigUint(_) => TraceValue::Normal,
            VariableValue::String(_) => TraceValue::Normal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_style_returns_normal() {
        let val = VariableValue::BigUint(42u32.into());
        let result = TraceValue::from_value(&val, Some(8), TraceStyle::Default);
        assert!(matches!(result, TraceValue::Normal));
    }

    #[test]
    fn test_dinotrace_all_zeros() {
        let val = VariableValue::BigUint(0u32.into());
        let result = TraceValue::from_value(&val, Some(8), TraceStyle::Dinotrace);
        assert!(matches!(result, TraceValue::AllZerosThick));
    }

    #[test]
    fn test_dinotrace_all_ones_8bit() {
        let val = VariableValue::BigUint(255u32.into()); // 0xFF = 8 bits all set
        let result = TraceValue::from_value(&val, Some(8), TraceStyle::Dinotrace);
        assert!(matches!(result, TraceValue::AllOnes));
    }

    #[test]
    fn test_dinotrace_all_ones_32bit() {
        let val = VariableValue::BigUint(u32::MAX.into());
        let result = TraceValue::from_value(&val, Some(32), TraceStyle::Dinotrace);
        assert!(matches!(result, TraceValue::AllOnes));
    }

    #[test]
    fn test_dinotrace_partial_value() {
        let val = VariableValue::BigUint(127u32.into()); // 0x7F = not all ones in 8 bits
        let result = TraceValue::from_value(&val, Some(8), TraceStyle::Dinotrace);
        assert!(matches!(result, TraceValue::Normal));
    }

    #[test]
    fn test_dinotrace_no_num_bits() {
        let val = VariableValue::BigUint(255u32.into());
        let result = TraceValue::from_value(&val, None, TraceStyle::Dinotrace);
        // Without num_bits, cannot determine if all ones, so returns Normal
        assert!(matches!(result, TraceValue::Normal));
    }

    #[test]
    fn test_zero_style_all_zeros() {
        let val = VariableValue::BigUint(0u32.into());
        let result = TraceValue::from_value(&val, Some(8), TraceStyle::Zero);
        assert!(matches!(result, TraceValue::AllZeros));
    }

    #[test]
    fn test_zero_style_nonzero() {
        let val = VariableValue::BigUint(42u32.into());
        let result = TraceValue::from_value(&val, Some(8), TraceStyle::Zero);
        assert!(matches!(result, TraceValue::Normal));
    }

    #[test]
    fn test_zero_style_all_ones() {
        let val = VariableValue::BigUint(255u32.into());
        let result = TraceValue::from_value(&val, Some(8), TraceStyle::Zero);
        // Zero style doesn't special-case all ones
        assert!(matches!(result, TraceValue::Normal));
    }

    #[test]
    fn test_string_value_is_normal() {
        let val = VariableValue::String("hello".to_string());
        let result = TraceValue::from_value(&val, Some(8), TraceStyle::Dinotrace);
        assert!(matches!(result, TraceValue::Normal));
    }
}

//! Entropy estimation for generated secrets.

/// Bits of entropy of `count` independent uniform choices among `choices` options.
#[must_use]
pub fn bits(choices: usize, count: usize) -> f64 {
    if choices <= 1 || count == 0 {
        return 0.0;
    }
    #[allow(clippy::cast_precision_loss)]
    let (choices, count) = (choices as f64, count as f64);
    count * choices.log2()
}

/// Human-friendly strength label for a number of entropy bits.
#[must_use]
pub fn strength_label(bits: f64) -> &'static str {
    match bits {
        b if b < 40.0 => "weak",
        b if b < 64.0 => "fair",
        b if b < 80.0 => "good",
        b if b < 128.0 => "strong",
        _ => "excellent",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_bits() {
        assert!((bits(2, 10) - 10.0).abs() < 1e-9);
        assert!((bits(7776, 6) - 77.548).abs() < 0.01);
    }

    #[test]
    fn degenerate_inputs_have_zero_entropy() {
        assert!(bits(1, 10).abs() < f64::EPSILON);
        assert!(bits(0, 10).abs() < f64::EPSILON);
        assert!(bits(10, 0).abs() < f64::EPSILON);
    }

    #[test]
    fn labels() {
        assert_eq!(strength_label(20.0), "weak");
        assert_eq!(strength_label(50.0), "fair");
        assert_eq!(strength_label(70.0), "good");
        assert_eq!(strength_label(100.0), "strong");
        assert_eq!(strength_label(130.0), "excellent");
    }
}

//! The little CBOR (RFC 8949) that `WebAuthn` needs: integers, byte and text
//! strings, and maps, in the canonical form (shortest lengths).

/// A CBOR value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cbor {
    /// An integer.
    Int(i64),
    /// A byte string.
    Bytes(Vec<u8>),
    /// A text string.
    Text(String),
    /// A map, written in the given order.
    Map(Vec<(Cbor, Cbor)>),
}

fn head(out: &mut Vec<u8>, major: u8, value: u64) {
    let major = major << 5;
    match value {
        0..=23 => out.push(major | u8::try_from(value).unwrap_or(0)),
        24..=0xff => {
            out.push(major | 0x18);
            out.push(u8::try_from(value).unwrap_or(0));
        }
        0x100..=0xffff => {
            out.push(major | 0x19);
            out.extend_from_slice(&u16::try_from(value).unwrap_or(0).to_be_bytes());
        }
        0x1_0000..=0xffff_ffff => {
            out.push(major | 0x1a);
            out.extend_from_slice(&u32::try_from(value).unwrap_or(0).to_be_bytes());
        }
        _ => {
            out.push(major | 0x1b);
            out.extend_from_slice(&value.to_be_bytes());
        }
    }
}

impl Cbor {
    /// The encoded bytes.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.write(&mut out);
        out
    }

    fn write(&self, out: &mut Vec<u8>) {
        match self {
            Self::Int(n) if *n >= 0 => head(out, 0, n.unsigned_abs()),
            Self::Int(n) => head(out, 1, n.unsigned_abs() - 1),
            Self::Bytes(b) => {
                head(out, 2, b.len() as u64);
                out.extend_from_slice(b);
            }
            Self::Text(t) => {
                head(out, 3, t.len() as u64);
                out.extend_from_slice(t.as_bytes());
            }
            Self::Map(entries) => {
                head(out, 5, entries.len() as u64);
                for (k, v) in entries {
                    k.write(out);
                    v.write(out);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Cbor::{Bytes, Int, Map, Text};

    // Examples from RFC 8949 appendix A.
    #[test]
    fn matches_the_rfc_examples() {
        let cases: [(super::Cbor, &str); 12] = [
            (Int(0), "00"),
            (Int(23), "17"),
            (Int(24), "1818"),
            (Int(1000), "1903e8"),
            (Int(1_000_000), "1a000f4240"),
            (Int(1_000_000_000_000), "1b000000e8d4a51000"),
            (Int(-1), "20"),
            (Int(-7), "26"),
            (Int(-1000), "3903e7"),
            (Bytes(vec![1, 2, 3, 4]), "4401020304"),
            (Text("IETF".into()), "6449455446"),
            (Map(vec![(Int(1), Int(2)), (Int(3), Int(4))]), "a201020304"),
        ];
        for (value, hex) in cases {
            assert_eq!(hex::encode(value.encode()), hex, "{value:?}");
        }
        assert_eq!(hex::encode(Bytes(vec![0; 300]).encode())[..6], *"59012c");
    }
}

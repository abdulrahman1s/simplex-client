//! HPACK prefix integers (RFC 7541 section 5.1) and the decoding error.
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("hpack: {0}")]
pub struct HpackError(pub &'static str);

/// Appends `value` with an `prefix_bits`-bit prefix; `flags` are the bits above the prefix.
pub fn encode_int(value: usize, prefix_bits: u8, flags: u8, out: &mut Vec<u8>) {
    let max = (1usize << prefix_bits) - 1;
    if value < max {
        out.push(flags | value as u8);
        return;
    }
    out.push(flags | max as u8);
    let mut rest = value - max;
    while rest >= 128 {
        out.push((rest % 128) as u8 | 0x80);
        rest /= 128;
    }
    out.push(rest as u8);
}

pub fn decode_int(buf: &[u8], pos: &mut usize, prefix_bits: u8) -> Result<usize, HpackError> {
    let max = (1u64 << prefix_bits) - 1;
    let first = *buf.get(*pos).ok_or(HpackError("truncated integer"))?;
    *pos += 1;
    // u64 and a shift checked before use, so a 32-bit `usize` never shifts by 32 or more.
    let mut value = u64::from(first) & max;
    if value < max {
        return to_usize(value);
    }
    let mut shift = 0u32;
    loop {
        let b = *buf.get(*pos).ok_or(HpackError("truncated integer"))?;
        *pos += 1;
        if shift > MAX_SHIFT {
            return Err(HpackError("integer too long"));
        }
        value += u64::from(b & 0x7f) << shift;
        shift += 7;
        if value > 1 << 31 {
            return Err(HpackError("integer too large"));
        }
        if b & 0x80 == 0 {
            return to_usize(value);
        }
    }
}

/// Five continuation bytes carry 35 bits, more than the 2^31 limit needs.
const MAX_SHIFT: u32 = 28;

fn to_usize(v: u64) -> Result<usize, HpackError> {
    usize::try_from(v).map_err(|_| HpackError("integer too large"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_int_refuses_a_sixth_continuation_byte_before_shifting_past_32_bits() {
        let overlong = [0x7f, 0x80, 0x80, 0x80, 0x80, 0x80, 0x00];
        assert_eq!(
            decode_int(&overlong, &mut 0, 7),
            Err(HpackError("integer too long"))
        );
        let five = [0x7f, 0x80, 0x80, 0x80, 0x80, 0x00];
        assert_eq!(decode_int(&five, &mut 0, 7), Ok(127));
    }
}

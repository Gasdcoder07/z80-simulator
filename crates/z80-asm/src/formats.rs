//! Object file formats: Intel HEX and the raw `.obj` binary image.

use crate::error::{AsmError, AsmResult};

/// A block of bytes destined for a given address.
pub type Segment = (u16, Vec<u8>);

const HEX_RECORD_BYTES: usize = 16;

/// Write segments as Intel HEX, the format the simulator and most EPROM
/// programmers exchange.
pub fn to_intel_hex(segments: &[Segment]) -> String {
    let mut out = String::new();

    for (base, bytes) in segments {
        for (index, chunk) in bytes.chunks(HEX_RECORD_BYTES).enumerate() {
            let address = base.wrapping_add((index * HEX_RECORD_BYTES) as u16);
            out.push_str(&record(address, 0x00, chunk));
            out.push('\n');
        }
    }

    // End-of-file record.
    out.push_str(&record(0, 0x01, &[]));
    out.push('\n');
    out
}

fn record(address: u16, kind: u8, data: &[u8]) -> String {
    let [hi, lo] = address.to_be_bytes();
    let mut checksum = (data.len() as u8).wrapping_add(hi).wrapping_add(lo).wrapping_add(kind);

    let mut line = format!(":{:02X}{address:04X}{kind:02X}", data.len());
    for byte in data {
        line.push_str(&format!("{byte:02X}"));
        checksum = checksum.wrapping_add(*byte);
    }
    line.push_str(&format!("{:02X}", checksum.wrapping_neg()));
    line
}

/// Parse an Intel HEX file into address/byte pairs.
///
/// Extended-address records are honoured, but anything above 64K is an error
/// because the Z80 cannot reach it.
pub fn from_intel_hex(text: &str) -> AsmResult<Vec<(u16, u8)>> {
    let mut bytes = Vec::new();
    let mut base: u32 = 0;

    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let number = index + 1;
        let body = line
            .strip_prefix(':')
            .ok_or_else(|| hex_error(number, "record does not start with ':'"))?;

        let raw = decode_hex(body).ok_or_else(|| hex_error(number, "invalid hex digits"))?;
        if raw.len() < 5 {
            return Err(hex_error(number, "record is too short"));
        }

        let count = raw[0] as usize;
        if raw.len() != count + 5 {
            return Err(hex_error(number, "record length does not match its count"));
        }
        if raw.iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte)) != 0 {
            return Err(hex_error(number, "checksum mismatch"));
        }

        let address = u16::from_be_bytes([raw[1], raw[2]]) as u32;
        let data = &raw[4..4 + count];

        match raw[3] {
            // Data.
            0x00 => {
                for (offset, byte) in data.iter().enumerate() {
                    let target = base + address + offset as u32;
                    if target > 0xFFFF {
                        return Err(hex_error(number, "address is above 64K"));
                    }
                    bytes.push((target as u16, *byte));
                }
            }
            // End of file.
            0x01 => break,
            // Extended segment address.
            0x02 if count == 2 => {
                base = (u16::from_be_bytes([data[0], data[1]]) as u32) << 4;
            }
            // Extended linear address.
            0x04 if count == 2 => {
                base = (u16::from_be_bytes([data[0], data[1]]) as u32) << 16;
            }
            // Start-address records carry no data for us.
            0x03 | 0x05 => {}
            kind => return Err(hex_error(number, format!("unsupported record type {kind:02X}"))),
        }
    }

    Ok(bytes)
}

fn hex_error(line: usize, message: impl Into<String>) -> AsmError {
    AsmError::Io(format!("Intel HEX line {line}: {}", message.into()))
}

fn decode_hex(text: &str) -> Option<Vec<u8>> {
    if text.len() % 2 != 0 {
        return None;
    }
    let bytes = text.as_bytes();
    (0..bytes.len() / 2)
        .map(|i| {
            let pair = std::str::from_utf8(&bytes[i * 2..i * 2 + 2]).ok()?;
            u8::from_str_radix(pair, 16).ok()
        })
        .collect()
}

/// Flatten segments into one contiguous binary image, as a `.obj` file.
/// Gaps between segments are filled with `fill`.
pub fn to_binary(segments: &[Segment], fill: u8) -> (u16, Vec<u8>) {
    let Some(start) = segments.iter().map(|(address, _)| *address).min() else {
        return (0, Vec::new());
    };
    let end = segments
        .iter()
        .map(|(address, bytes)| *address as usize + bytes.len())
        .max()
        .unwrap_or(0);

    let mut image = vec![fill; end - start as usize];
    for (address, bytes) in segments {
        let offset = *address as usize - start as usize;
        image[offset..offset + bytes.len()].copy_from_slice(bytes);
    }

    (start, image)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_round_trip() {
        let segments = vec![(0x0100, (0u8..=40).collect::<Vec<u8>>()), (0x8000, vec![0xAA, 0xBB])];
        let text = to_intel_hex(&segments);
        let parsed = from_intel_hex(&text).unwrap();

        let mut expected = Vec::new();
        for (base, bytes) in &segments {
            for (offset, byte) in bytes.iter().enumerate() {
                expected.push((base + offset as u16, *byte));
            }
        }
        assert_eq!(parsed, expected);
    }

    #[test]
    fn hex_checksums_are_verified() {
        let good = ":03010000010203F6\n:00000001FF\n";
        assert!(from_intel_hex(good).is_ok());
        let bad = ":03010000010203F7\n:00000001FF\n";
        assert!(from_intel_hex(bad).is_err());
    }
}

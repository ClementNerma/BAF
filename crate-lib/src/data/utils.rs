use anyhow::{bail, Context, Result};

use crate::source::ReadableSource;

/// Decode an UTF-8 name from an archive
/// Structure: see [`encode_name`]
pub fn decode_name_from_source(source: &mut impl ReadableSource) -> Result<String> {
    // TODO: in case of invalid name, return a diagnostic instead of an error
    decode_name(
        source.consume_next_value::<u8>()?,
        source.consume_next(255)?,
    )
}

/// Decode an UTF-8 name from an archive
/// Structure: see [`encode_name`]
pub fn decode_name(name_len: u8, name: Vec<u8>) -> Result<String> {
    let mut name = name.to_vec();
    name.shrink_to(usize::from(name_len));

    let name = String::from_utf8(name).context("Failed to decode name as UTF-8")?;

    check_name(&name)?;

    Ok(name)
}

/// Encode an UTF-8 for an archive
/// Structure: <length (1 byte)> then <name> (up to 255 bytes, unused ones filled with zeroes)
///
/// Will faill if the name is empty, longer than 255 bytes, or contains invalid symbols
/// (e.g. forward and backward slashes, newline symbols)
pub fn encode_name(name: &str) -> Result<[u8; 256]> {
    check_name(name)?;

    let mut bytes = [0; 256];

    bytes[0] = u8::try_from(name.len()).unwrap();
    bytes[1..=name.len()].copy_from_slice(name.as_bytes());

    Ok(bytes)
}

/// Check if a name is a valid archive item's name
pub fn check_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("Name cannot be empty");
    }

    if name.len() > 255 {
        bail!("Name cannot be longer than 255 bytes");
    }

    if name.contains('/') {
        bail!("Name contains forbidden '/' symbol");
    }

    if name.contains('\\') {
        bail!("Name contains forbidden '\\' symbol");
    }

    if name.contains('\n') || name.contains('\r') {
        bail!("Name contains forbidden return line symbol");
    }

    Ok(())
}

/// Wrap the provided number in [`Some`] if it is not equal to zero, return [`None`] otherwise
pub fn none_if_zero(value: u64) -> Option<u64> {
    if value != 0 {
        Some(value)
    } else {
        None
    }
}

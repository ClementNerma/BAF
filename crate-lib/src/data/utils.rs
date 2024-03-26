use anyhow::{bail, Context, Result};

use crate::source::ReadableSource;

pub fn decode_name(source: &mut impl ReadableSource) -> Result<String> {
    let name_len = source.consume_next_value::<u8>()?;

    let mut name = source.consume_next(255)?;

    name.truncate(name_len.into());

    String::from_utf8(name).context("Failed to decode name as UTF-8")
}

pub fn encode_name(name: &str) -> Result<[u8; 256]> {
    if name.is_empty() {
        bail!("Name cannot be empty");
    }

    if name.len() > 255 {
        bail!("Name cannot be longer than 255 bytes");
    }

    let mut bytes = [0; 256];

    bytes[0] = u8::try_from(name.len()).unwrap();
    bytes[1..=name.len()].copy_from_slice(name.as_bytes());

    Ok(bytes)
}

pub fn none_if_zero(value: u64) -> Option<u64> {
    if value != 0 {
        Some(value)
    } else {
        None
    }
}

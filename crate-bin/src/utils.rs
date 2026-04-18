use std::time::SystemTime;

use baf::Timestamp;
use jiff::fmt::rfc2822::DateTimePrinter;

/// Convert a size in bytes to a human-readable string with the specified precision
pub fn human_size(size: u64, precision: Option<u8>) -> String {
    let units = ["B", "KiB", "MiB", "GiB", "TiB"];

    let (unit, unit_base) = units
        .iter()
        .enumerate()
        .rev()
        .find_map(|(i, unit)| {
            let base = 1024_u64.pow(i.try_into().unwrap());

            if size >= base || base == 1 {
                Some((unit, base))
            } else {
                None
            }
        })
        .unwrap();

    format!(
        "{} {unit}",
        approx_int_div(size, unit_base, precision.unwrap_or(2))
    )
}

/// Perform an approximate integer division
///
/// The last decimal will be rounded to the nearest.
///
/// The `precision` parameter is the number of floating-point decimals to keep.
fn approx_int_div(a: u64, b: u64, precision: u8) -> String {
    let max_prec = 10_u128.pow(u32::from(precision));

    let div = u128::from(a) * max_prec * 10 / u128::from(b);
    let div = (div / 10) + if div % 10 >= 5 { 1 } else { 0 };

    let int_part = div / max_prec;
    let frac_part = div % max_prec;

    let mut out = int_part.to_string();

    if frac_part > 0 && precision > 0 {
        out.push('.');
        out.push_str(&format!(
            "{:#0precision$}",
            frac_part,
            precision = precision.into()
        ));
    }

    out
}

/// Convert a timestamp in milliseconds since the UNIX epoch to a human-readable string
pub fn human_time(timestamp: Timestamp) -> String {
    let Ok(zdt) = jiff::Zoned::try_from(SystemTime::from(timestamp)) else {
        return "<invalid timestamp>".to_string();
    };

    let mut buf = String::new();

    DateTimePrinter::new().print_zoned(&zdt, &mut buf).unwrap();

    buf
}

use std::{
    io::Read,
    time::{Duration, SystemTime},
};

use anyhow::Result;

use crate::source::{FromSourceBytes, Source};

/// Representation of a timestamp
///
/// Stores the number of seconds elapsed since Unix's EPOCH
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp(u64);

impl Timestamp {
    /// Get the current timestamp
    pub fn now() -> Self {
        Self::from(SystemTime::now())
    }

    pub(crate) fn encode(&self) -> [u8; 8] {
        self.0.to_le_bytes()
    }
}

impl From<SystemTime> for Timestamp {
    fn from(value: SystemTime) -> Self {
        Self(
            value
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        )
    }
}

impl From<Timestamp> for SystemTime {
    fn from(value: Timestamp) -> Self {
        SystemTime::UNIX_EPOCH + Duration::from_secs(value.0)
    }
}

impl FromSourceBytes for Timestamp {
    fn read_from(source: &mut Source<impl Read>) -> Result<Self>
    where
        Self: Sized,
    {
        source.read_value::<u64>().map(Self)
    }
}

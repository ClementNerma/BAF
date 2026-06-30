use std::{
    io::Read,
    time::{Duration, SystemTime},
};

use thiserror::Error;

use crate::source::{FromSourceBytes, Source};

/// Representation of a timestamp
///
/// Stores the number of seconds elapsed since Unix's EPOCH
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp(u64);

/// Error that can occur when constructing a [`Timestamp`]
#[derive(Debug, Clone, Error)]
pub enum TimestampError {
    /// The provided [`SystemTime`] precedes the Unix epoch (Jan 1, 1970)
    #[error("timestamp precedes the Unix epoch")]
    BeforeEpoch,
}

impl Timestamp {
    /// Get the current timestamp
    pub(crate) fn now() -> Self {
        Self::try_from(SystemTime::now()).expect("SystemTime::now() is always after the Unix epoch")
    }

    pub(crate) fn encode(&self) -> [u8; 8] {
        self.0.to_le_bytes()
    }
}

impl TryFrom<SystemTime> for Timestamp {
    type Error = TimestampError;

    fn try_from(value: SystemTime) -> Result<Self, Self::Error> {
        let secs = value
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(|_| TimestampError::BeforeEpoch)?
            .as_secs();
        Ok(Self(secs))
    }
}

impl From<Timestamp> for SystemTime {
    fn from(value: Timestamp) -> Self {
        SystemTime::UNIX_EPOCH + Duration::from_secs(value.0)
    }
}

impl FromSourceBytes for Timestamp {
    fn read_from(source: &mut Source<impl Read>) -> std::io::Result<Self>
    where
        Self: Sized,
    {
        source.read_value::<u64>().map(Self)
    }
}

use std::{
    io::Read,
    time::{Duration, SystemTime},
};

use anyhow::Result;

use crate::source::{FromSourceBytes, Source};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp(u64);

impl Timestamp {
    pub fn now() -> Self {
        Self::from(SystemTime::now())
    }

    pub fn encode(&self) -> [u8; 8] {
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

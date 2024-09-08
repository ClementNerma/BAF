use std::time::{Duration, SystemTime};

use anyhow::Result;

use crate::source::{ConsumableSource, FromSourceBytes};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp(u64);

impl Timestamp {
    pub fn now() -> Self {
        Self::from(SystemTime::now())
    }

    pub fn secs_since_epoch(&self) -> u64 {
        self.0
    }

    pub fn system_time(&self) -> SystemTime {
        SystemTime::from(*self)
    }

    pub fn encode(&self) -> [u8; 8] {
        self.0.to_be_bytes()
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
    fn decode(source: &mut impl ConsumableSource) -> Result<Self>
    where
        Self: Sized,
    {
        source.consume_next_value::<u64>().map(Self)
    }
}

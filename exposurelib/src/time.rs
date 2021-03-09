use crate::primitives::TekRollingPeriod;
use chrono::prelude::*;
use serde::{Deserialize, Serialize};
use std::ops::{Add, Sub};
use std::collections::BTreeSet;

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct ExposureTime {
    en_interval_number: u32,
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct TekrpMultiple {
    en_interval_number: u32,
}

impl TekrpMultiple {
    pub fn from_exposure_time(exposure_time: ExposureTime, tekrp: TekRollingPeriod) -> Self {
        let tekrp = u32::from(tekrp);
        Self { en_interval_number: u32::from(exposure_time) / tekrp * tekrp }
    }
}

impl ExposureTime {
    pub fn as_number(&self) -> u32 {
        self.en_interval_number
    }
    pub fn as_bytes(&self) -> [u8; std::mem::size_of::<u32>()] {
        self.en_interval_number.to_le_bytes()
    }
    pub fn floor_tekrp_multiple(&self, tekrp: TekRollingPeriod) -> Self {
        let tekrp = u32::from(tekrp);
        ExposureTime {
            en_interval_number: self.en_interval_number / tekrp * tekrp,
        }
    }
    pub fn en_interval_number(utc: DateTime<Utc>) -> u32 {
        (utc.timestamp() / (60 * 10)) as u32
    }
}

impl From<ExposureTime> for u32 {
    fn from(exposre_time: ExposureTime) -> Self {
        exposre_time.en_interval_number
    }
}

impl From<u32> for ExposureTime {
    fn from(en_interval_number: u32) -> Self {
        Self { en_interval_number }
    }
}

impl From<DateTime<Utc>> for ExposureTime {
    fn from(utc: DateTime<Utc>) -> Self {
        Self {
            en_interval_number: Self::en_interval_number(utc),
        }
    }
}

impl PartialEq for ExposureTime {
    fn eq(&self, other: &Self) -> bool {
        self.en_interval_number == other.en_interval_number
    }
}

impl Eq for ExposureTime {}

impl PartialOrd for ExposureTime {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ExposureTime {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.en_interval_number.cmp(&other.en_interval_number)
    }
}

impl Add for ExposureTime {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            en_interval_number: self.en_interval_number + other.en_interval_number,
        }
    }
}

impl Add<TekRollingPeriod> for ExposureTime {
    type Output = Self;

    fn add(self, other: TekRollingPeriod) -> Self {
        Self {
            en_interval_number: self.en_interval_number + u32::from(other),
        }
    }
}

impl Sub for ExposureTime {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self {
            en_interval_number: self.en_interval_number - other.en_interval_number,
        }
    }
}

impl Sub<TekRollingPeriod> for ExposureTime {
    type Output = Self;

    fn sub(self, other: TekRollingPeriod) -> Self {
        Self {
            en_interval_number: self.en_interval_number - u32::from(other),
        }
    }
}

pub trait ExposureTimeInterval
where
    Self: Sized,
{
    fn contains(&self, exposure_time: ExposureTime) -> bool;
    fn intersection(&self, other: &Self) -> Option<Self>;
}

pub struct ExposureTimeSet {
    inner: BTreeSet<ExposureTime>,
}

pub struct ContinuousExposureTimeInterval {
    from_inclusive: ExposureTime,
    to_exclusive: ExposureTime,
}

impl ContinuousExposureTimeInterval {
    pub fn new(from_inclusive: ExposureTime, to_exclusive: ExposureTime) -> Self {
        Self {
            from_inclusive,
            to_exclusive,
        }
    }
}

impl ExposureTimeInterval for ContinuousExposureTimeInterval {
    fn contains(&self, exposure_time: ExposureTime) -> bool {
        self.from_inclusive <= exposure_time && exposure_time < self.to_exclusive
    }
    fn intersection(&self, other: &Self) -> Option<Self> {
        if self.from_inclusive >= other.to_exclusive || self.to_exclusive <= other.from_inclusive {
            return None;
        }
        let from_inclusive = std::cmp::max(self.from_inclusive, other.from_inclusive);
        let to_exclusive = if self.to_exclusive == other.to_exclusive {
            self.to_exclusive
        } else {
            std::cmp::min(self.to_exclusive, other.to_exclusive) + ExposureTime::from(1)
        };
        Some(Self {
            from_inclusive,
            to_exclusive,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_exposure_time_creation() {
        let exposure_time: ExposureTime = Utc.timestamp(0, 0).into();
        assert_eq!(exposure_time.as_number(), 0);
        let exposure_time: ExposureTime = Utc.timestamp(10 * 60, 0).into();
        assert_eq!(exposure_time.as_number(), 1);
        let exposure_time: ExposureTime = Utc.timestamp(9 * 60, 999).into();
        assert_eq!(exposure_time.as_number(), 0);
        let exposure_time: ExposureTime = Utc.ymd(1970, 1, 1).and_hms(0, 0, 0).into();
        assert_eq!(exposure_time.as_number(), 0);
        let exposure_time: ExposureTime = Utc.ymd(1970, 1, 2).and_hms(0, 0, 0).into();
        assert_eq!(exposure_time.as_number(), 24 * 60 / 10);
        let exposure_time: ExposureTime = Utc.ymd(1970, 1, 2).and_hms(0, 2, 0).into();
        assert_eq!(exposure_time.as_number(), 24 * 60 / 10);
        let exposure_time: ExposureTime = Utc.ymd(1970, 1, 2).and_hms(0, 9, 59).into();
        assert_eq!(exposure_time.as_number(), 24 * 60 / 10);
        let exposure_time: ExposureTime = Utc.ymd(1970, 1, 2).and_hms(0, 10, 0).into();
        assert_eq!(exposure_time.as_number(), 24 * 60 / 10 + 1);
    }

    #[test]
    fn test_floor_tekrp_multiple() {
        let tekrp = TekRollingPeriod::default();
        let exposure_time = ExposureTime::from(Utc::now());
        let tekrp_multiple = exposure_time.floor_tekrp_multiple(tekrp);
        // floor_tekrp_multiple() must be idempotent (!)
        assert_eq!(tekrp_multiple, tekrp_multiple.floor_tekrp_multiple(tekrp));
    }

    #[test]
    fn test_exposure_time_comparison() {
        let smaller = ExposureTime::from(Utc::now() - Duration::minutes(11));
        let bigger = ExposureTime::from(Utc::now());
        assert!(smaller < bigger);

        let equal_a = ExposureTime::from(Utc.ymd(2021, 02, 17).and_hms(0, 45, 0));
        let equal_b = ExposureTime::from(Utc.ymd(2021, 02, 17).and_hms(0, 46, 0));
        assert!(equal_a == equal_b);
    }

    #[test]
    fn test_continouos_interval() {
        let tekrp = TekRollingPeriod::default();
        let exposure_time = ExposureTime::from(1000);
        let continouos_interval =
            ContinuousExposureTimeInterval::new(exposure_time, exposure_time + tekrp);
        assert!(continouos_interval.contains(exposure_time));
        assert!(continouos_interval.contains(exposure_time + tekrp - ExposureTime::from(1)));
        assert!(!continouos_interval.contains(exposure_time + tekrp));
        assert!(continouos_interval.contains(ExposureTime::from(1005)));

        let non_overlapping_lower =
            ContinuousExposureTimeInterval::new(exposure_time - tekrp, exposure_time);
        assert!(continouos_interval
            .intersection(&non_overlapping_lower)
            .is_none());

        let non_overlapping_higher = ContinuousExposureTimeInterval::new(
            exposure_time + tekrp,
            exposure_time + tekrp + tekrp,
        );
        assert!(continouos_interval
            .intersection(&non_overlapping_higher)
            .is_none());
    }
}

use crate::primitives::TekRollingPeriod;
use chrono::prelude::*;
use chrono::Duration;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;
use std::ops::{Add, Sub};

#[derive(Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ExposureTime {
    en_interval_number: u32,
}

impl ExposureTime {
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

impl From<ExposureTime> for [u8; std::mem::size_of::<u32>()] {
    fn from(exposure_time: ExposureTime) -> Self {
        exposure_time.en_interval_number.to_le_bytes()
    }
}

impl From<ExposureTime> for u32 {
    fn from(exposure_time: ExposureTime) -> Self {
        exposure_time.en_interval_number
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

impl fmt::Debug for ExposureTime {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ENIntervalNumber({})", self.en_interval_number)
    }
}

pub type ExposureTimeSet = BTreeSet<ExposureTime>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimeInterval {
    from_including: DateTime<Utc>,
    to_excluding: DateTime<Utc>,
}

impl TimeInterval {
    pub fn with_bounds(from_including: DateTime<Utc>, to_excluding: DateTime<Utc>) -> Self {
        if to_excluding <= from_including {
            panic!("Invalid range specified.");
        }
        Self {
            from_including,
            to_excluding,
        }
    }
    pub fn with_duration(from: DateTime<Utc>, duration: Duration) -> Self {
        if duration <= Duration::seconds(0) {
            panic!("Negative duration given.");
        }
        Self {
            from_including: from,
            to_excluding: from + duration,
        }
    }
    pub fn with_alignment(duration: Duration) -> Self {
        if duration <= Duration::seconds(0) {
            panic!("Negative duration given.");
        }
        let mut from_including = Utc::today().and_hms(0, 0, 0);
        loop {
            let candidate = Self::with_duration(from_including, duration);
            if candidate.contains(&Utc::now()) {
                return candidate;
            } else {
                from_including = from_including + duration;
            }
        }
    }
    pub fn next_interval(&self) -> Self {
        let from_including = self.to_excluding;
        Self {
            from_including,
            to_excluding: from_including + self.duration(),
        }
    }
    pub fn contains(&self, time: &DateTime<Utc>) -> bool {
        !self.before(time) && !self.after(time)
    }
    pub fn before(&self, time: &DateTime<Utc>) -> bool {
        self.from_including - *time > Duration::zero()
    }
    pub fn after(&self, time: &DateTime<Utc>) -> bool {
        *time - self.to_excluding >= Duration::zero()
    }
    pub fn duration(&self) -> Duration {
        self.to_excluding - self.from_including
    }
    pub fn from_including(&self) -> &DateTime<Utc> {
        &self.from_including
    }
    pub fn to_excluding(&self) -> &DateTime<Utc> {
        &self.to_excluding
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_exposure_time_creation() {
        let exposure_time: ExposureTime = Utc.timestamp(0, 0).into();
        assert_eq!(u32::from(exposure_time), 0);
        let exposure_time: ExposureTime = Utc.timestamp(10 * 60, 0).into();
        assert_eq!(u32::from(exposure_time), 1);
        let exposure_time: ExposureTime = Utc.timestamp(9 * 60, 999).into();
        assert_eq!(u32::from(exposure_time), 0);
        let exposure_time: ExposureTime = Utc.ymd(1970, 1, 1).and_hms(0, 0, 0).into();
        assert_eq!(u32::from(exposure_time), 0);
        let exposure_time: ExposureTime = Utc.ymd(1970, 1, 2).and_hms(0, 0, 0).into();
        assert_eq!(u32::from(exposure_time), 24 * 60 / 10);
        let exposure_time: ExposureTime = Utc.ymd(1970, 1, 2).and_hms(0, 2, 0).into();
        assert_eq!(u32::from(exposure_time), 24 * 60 / 10);
        let exposure_time: ExposureTime = Utc.ymd(1970, 1, 2).and_hms(0, 9, 59).into();
        assert_eq!(u32::from(exposure_time), 24 * 60 / 10);
        let exposure_time: ExposureTime = Utc.ymd(1970, 1, 2).and_hms(0, 10, 0).into();
        assert_eq!(u32::from(exposure_time), 24 * 60 / 10 + 1);
    }

    #[test]
    fn test_floor_tekrp_multiple() {
        let tekrp = TekRollingPeriod::default();
        let exposure_time = ExposureTime::from(Utc::now());
        let tekrp_multiple = exposure_time.floor_tekrp_multiple(tekrp);
        assert_eq!(
            tekrp_multiple,
            tekrp_multiple.floor_tekrp_multiple(tekrp),
            "floor_tekrp_multiple() is *not* idempotent"
        );
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
    fn test_time_interval() {
        let chunk_period = Duration::seconds(30);

        let interval = TimeInterval::with_duration(Utc::now(), chunk_period);
        assert_eq!(chunk_period, interval.duration());

        let from_including = Utc.ymd(2021, 03, 20).and_hms(12, 00, 00);
        let to_excluding = Utc.ymd(2021, 03, 20).and_hms(12, 30, 00);
        let interval = TimeInterval::with_bounds(from_including, to_excluding);
        assert_eq!(Duration::minutes(30), interval.duration());
        assert!(!interval.before(&from_including));
        assert!(!interval.after(&from_including));
        assert!(!interval.before(&&to_excluding));
        assert!(interval.after(&to_excluding));
        assert!(interval.contains(&from_including));
        assert!(!interval.contains(&to_excluding));
        assert!(interval.contains(&Utc.ymd(2021, 03, 20).and_hms(12, 15, 00)));
        assert!(!interval.contains(&Utc.ymd(2021, 03, 20).and_hms(11, 59, 59)));

        let next_interval = interval.next_interval();
        assert_eq!(Duration::minutes(30), next_interval.duration());
        assert_eq!(
            Utc.ymd(2021, 03, 20).and_hms(12, 30, 00),
            *next_interval.from_including()
        );
        assert_eq!(
            Utc.ymd(2021, 03, 20).and_hms(13, 00, 00),
            *next_interval.to_excluding()
        );

        let chunk_period = Duration::minutes(30);
        let interval = TimeInterval::with_alignment(chunk_period);
        assert!(interval.contains(&Utc::now()));
        let mut intervals_from_midnight = 0;
        let midnight = Utc::today().and_hms(0, 0, 0);
        let from_including = interval.from_including();
        let to_excluding = interval.to_excluding();
        loop {
            if midnight + (chunk_period * intervals_from_midnight) == *from_including {
                break;
            }
            intervals_from_midnight += 1;
        }
        assert_eq!(*from_including + chunk_period, *to_excluding);
    }

    #[test]
    #[should_panic]
    fn test_panic_time_interval_creation_1() {
        TimeInterval::with_duration(Utc::now(), Duration::seconds(-30));
    }

    #[test]
    #[should_panic]
    fn test_panic_time_interval_creation_2() {
        TimeInterval::with_duration(Utc::now(), Duration::seconds(0));
    }

    #[test]
    #[should_panic]
    fn test_panic_time_interval_creation_3() {
        let now = Utc::now();
        TimeInterval::with_bounds(now, now);
    }

    #[test]
    #[should_panic]
    fn test_panic_time_interval_creation_4() {
        let now = Utc::now();
        TimeInterval::with_bounds(now, now - Duration::seconds(10));
    }
}

use chrono::prelude::*;

pub struct ExposureTime {
    en_interval_number: u32,
}

impl ExposureTime {
    pub fn en_interval_number(&self) -> u32 {
        self.en_interval_number
    }
}

impl From<DateTime<Utc>> for ExposureTime {
    fn from(utc: DateTime<Utc>) -> Self {
        Self {
            en_interval_number: (utc.timestamp() / (60 * 10)) as u32,
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

// Do I really need this? I'd rather go for a non-continouos time range..
pub struct ExposureTimeRange {}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_exposure_time_creation() {
        let exposure_time: ExposureTime = Utc.timestamp(0, 0).into();
        assert_eq!(exposure_time.en_interval_number(), 0);
        let exposure_time: ExposureTime = Utc.timestamp(10 * 60, 0).into();
        assert_eq!(exposure_time.en_interval_number(), 1);
        let exposure_time: ExposureTime = Utc.timestamp(9 * 60, 999).into();
        assert_eq!(exposure_time.en_interval_number(), 0);
        let exposure_time: ExposureTime = Utc.ymd(1970, 1, 1).and_hms(0, 0, 0).into();
        assert_eq!(exposure_time.en_interval_number(), 0);
        let exposure_time: ExposureTime = Utc.ymd(1970, 1, 2).and_hms(0, 0, 0).into();
        assert_eq!(exposure_time.en_interval_number(), 24 * 60 / 10);
        let exposure_time: ExposureTime = Utc.ymd(1970, 1, 2).and_hms(0, 2, 0).into();
        assert_eq!(exposure_time.en_interval_number(), 24 * 60 / 10);
        let exposure_time: ExposureTime = Utc.ymd(1970, 1, 2).and_hms(0, 9, 59).into();
        assert_eq!(exposure_time.en_interval_number(), 24 * 60 / 10);
        let exposure_time: ExposureTime = Utc.ymd(1970, 1, 2).and_hms(0, 10, 0).into();
        assert_eq!(exposure_time.en_interval_number(), 24 * 60 / 10 + 1);
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
}

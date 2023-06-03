mod request;
mod reservation;
mod reservation_query;
mod reservation_status;

use chrono::{DateTime, Utc};
use prost_types::Timestamp;
use sqlx::postgres::types::PgRange;
use std::ops::Bound;

use crate::{convert_to_utc_time, Error};

pub fn get_timespan(start: Option<&Timestamp>, end: Option<&Timestamp>) -> PgRange<DateTime<Utc>> {
    let start = convert_to_utc_time(start.as_ref().unwrap());
    let end = convert_to_utc_time(end.as_ref().unwrap());

    PgRange {
        start: Bound::Included(start),
        end: Bound::Excluded(end),
    }
}

pub fn validate_range(start: Option<&Timestamp>, end: Option<&Timestamp>) -> Result<(), Error> {
    if start.is_none() || end.is_none() {
        return Err(Error::InvalidTimespan);
    }

    let start = start.unwrap();
    let end = end.unwrap();

    if start.seconds >= end.seconds {
        return Err(Error::InvalidTimespan);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_range_should_work() {
        let start = Some(Timestamp {
            seconds: 0,
            nanos: 0,
        });
        let end = Some(Timestamp {
            seconds: 1,
            nanos: 0,
        });

        assert!(validate_range(start.as_ref(), end.as_ref()).is_ok());
    }

    #[test]
    fn validate_range_should_reject_invalid_range() {
        let start = Some(Timestamp {
            seconds: 1,
            nanos: 0,
        });
        let end = Some(Timestamp {
            seconds: 0,
            nanos: 0,
        });

        assert!(validate_range(start.as_ref(), end.as_ref()).is_err());
    }

    #[test]
    fn get_timespan_should_work() {
        let start = Some(Timestamp {
            seconds: 0,
            nanos: 0,
        });
        let end = Some(Timestamp {
            seconds: 1,
            nanos: 0,
        });

        let timespan = get_timespan(start.as_ref(), end.as_ref());

        assert_eq!(
            timespan.start,
            Bound::Included(convert_to_utc_time(start.as_ref().unwrap()))
        );
        assert_eq!(
            timespan.end,
            Bound::Excluded(convert_to_utc_time(end.as_ref().unwrap()))
        );
    }
}

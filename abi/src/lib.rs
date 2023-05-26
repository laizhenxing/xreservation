mod pb;

use chrono::{DateTime, NaiveDateTime, Utc};
use prost_types::Timestamp;

pub use pb::*;

pub fn convert_to_utc_time(ts: Timestamp) -> DateTime<Utc> {
    let naive = NaiveDateTime::from_timestamp_opt(ts.seconds, ts.nanos as _).unwrap();
    DateTime::<Utc>::from_utc(naive, Utc)
}

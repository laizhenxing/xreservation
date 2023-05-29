use chrono::{DateTime, Utc};
use sqlx::postgres::types::PgRange;

use super::{get_timespan, validate_range};
use crate::{Error, ReservationQuery, Validator};

impl ReservationQuery {
    pub fn get_timespan(&self) -> PgRange<DateTime<Utc>> {
        get_timespan(self.start.as_ref(), self.end.as_ref())
    }
}

impl Validator for ReservationQuery {
    fn validate(&self) -> Result<(), Error> {
        if self.user_id.is_empty() {
            return Err(Error::InvalidUserId(self.user_id.clone()));
        }

        //if self.resource_id.is_empty() {
        //    return Err(Error::InvalidResourceId(self.resource_id.clone()));
        //}

        validate_range(self.start.as_ref(), self.end.as_ref())
    }
}

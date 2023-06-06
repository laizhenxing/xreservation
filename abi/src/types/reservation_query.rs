use prost_types::Timestamp;

use crate::{
    convert_to_utc_time, Error, Normalizer, ReservationQuery, ReservationStatus, ToSql, Validator,
};

impl ReservationQuery {
    pub fn get_status(&self) -> ReservationStatus {
        ReservationStatus::from_i32(self.status).unwrap()
    }
}

impl Validator for ReservationQuery {
    fn validate(&self) -> Result<(), Error> {
        ReservationStatus::from_i32(self.status).ok_or(Error::InvalidStatus(self.status))?;

        if let (Some(start), Some(end)) = (self.start.as_ref(), self.end.as_ref()) {
            if start.seconds > end.seconds {
                return Err(Error::InvalidTimespan);
            }
        }

        Ok(())
    }
}

impl Normalizer for ReservationQuery {
    fn do_normalize(&mut self) {
        if self.status == ReservationStatus::Unknown as i32 {
            self.status = ReservationStatus::Pending as i32;
        }
    }
}

impl ToSql for ReservationQuery {
    fn to_sql(&self) -> String {
        let status = ReservationStatus::from_i32(self.status).unwrap();

        let timespan = format!(
            "tstzrange('{}', '{}')",
            get_time_string(self.start.as_ref(), true),
            get_time_string(self.end.as_ref(), false),
        );

        let condition = match (self.user_id.is_empty(), self.resource_id.is_empty()) {
            (true, true) => "TRUE".into(),
            (false, true) => format!("user_id = '{}'", self.user_id),
            (true, false) => format!("resource_id = '{}'", self.resource_id),
            (false, false) => format!(
                "user_id = '{}' AND resource_id = '{}'",
                self.user_id, self.resource_id
            ),
        };

        let direction = if !self.desc { "ASC" } else { "DESC" };

        format!("SELECT * FROM rsvp.reservations WHERE {} @> timespan AND status = '{}'::rsvp.reservation_status AND {} ORDER BY lower(timespan) {}", timespan, status, condition, direction)
    }
}

fn get_time_string(ts: Option<&Timestamp>, start: bool) -> String {
    match ts {
        Some(ts) => convert_to_utc_time(ts).to_rfc3339(),
        None => (if start { "-infinity" } else { "infinity" }).into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ReservationQueryBuilder;

    #[test]
    fn query_to_sql_should_work() {
        let mut query = ReservationQueryBuilder::default()
            .user_id("user")
            .resource_id("resource")
            .start(Timestamp {
                seconds: 0,
                nanos: 0,
            })
            .end(Timestamp {
                seconds: 1,
                nanos: 0,
            })
            .build()
            .unwrap();
        query.do_normalize();

        assert_eq!(
            query.to_sql(),
            "SELECT * FROM rsvp.reservations WHERE tstzrange('1970-01-01T00:00:00+00:00', '1970-01-01T00:00:01+00:00') @> timespan AND status = 'pending'::rsvp.reservation_status AND user_id = 'user' AND resource_id = 'resource' ORDER BY lower(timespan) ASC"
        );

        let query = ReservationQueryBuilder::default()
            .user_id("user")
            .resource_id("resource")
            .status(ReservationStatus::Pending)
            .desc(true)
            .build()
            .unwrap();

        assert_eq!(
            query.to_sql(),
            "SELECT * FROM rsvp.reservations WHERE tstzrange('-infinity', 'infinity') @> timespan AND status = 'pending'::rsvp.reservation_status AND user_id = 'user' AND resource_id = 'resource' ORDER BY lower(timespan) DESC"
        );

        let query = ReservationQueryBuilder::default()
            .user_id("user")
            .resource_id("resource")
            .status(ReservationStatus::Pending)
            .desc(true)
            .start(Timestamp {
                seconds: 0,
                nanos: 0,
            })
            .build()
            .unwrap();
        assert_eq!(
            query.to_sql(),
            "SELECT * FROM rsvp.reservations WHERE tstzrange('1970-01-01T00:00:00+00:00', 'infinity') @> timespan AND status = 'pending'::rsvp.reservation_status AND user_id = 'user' AND resource_id = 'resource' ORDER BY lower(timespan) DESC"
        );

        let query = ReservationQueryBuilder::default()
            .user_id("user")
            .resource_id("resource")
            .status(ReservationStatus::Pending)
            .desc(true)
            .end(Timestamp {
                seconds: 1,
                nanos: 0,
            })
            .build()
            .unwrap();
        assert_eq!(
            query.to_sql(),
            "SELECT * FROM rsvp.reservations WHERE tstzrange('-infinity', '1970-01-01T00:00:01+00:00') @> timespan AND status = 'pending'::rsvp.reservation_status AND user_id = 'user' AND resource_id = 'resource' ORDER BY lower(timespan) DESC"
        );
    }
}

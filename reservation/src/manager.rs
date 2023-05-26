use crate::{ReservationError, ReservationManager, Rsvp};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{postgres::types::PgRange, Row};

#[async_trait]
impl Rsvp for ReservationManager {
    async fn reserve(
        &self,
        mut rsvp: abi::Reservation,
    ) -> Result<abi::Reservation, ReservationError> {
        if rsvp.start.is_none() || rsvp.end.is_none() {
            return Err(ReservationError::InvalidTimespan);
        }

        // if rsvp.start.unwrap() >= rsvp.end.unwrap() {
        //     return Err(ReservationError::InvalidTimespan);
        // }

        let start = abi::convert_to_utc_time(rsvp.start.as_ref().unwrap().clone());
        let end = abi::convert_to_utc_time(rsvp.end.as_ref().unwrap().clone());
        let timespan: PgRange<DateTime<Utc>> = (start..end).into();

        let status = abi::ReservationStatus::from_i32(rsvp.status)
            .unwrap_or(abi::ReservationStatus::Pending);

        let sql = "INSERT INTO reservation (user_id, resource_id, timespan, note, status)
            VALUES ($1, $2, $3, $4, $5) RETURNING id";
        let id = sqlx::query(sql)
            .bind(rsvp.user_id.clone())
            .bind(rsvp.resource_id.clone())
            .bind(timespan)
            .bind(rsvp.note.clone())
            .bind(status)
            .fetch_one(&self.pool)
            .await?
            .get(0);
        rsvp.id = id;

        Ok(rsvp)
    }

    async fn change_status(
        &self,
        _rsvp: abi::Reservation,
    ) -> Result<abi::Reservation, ReservationError> {
        todo!()
    }

    async fn update_note(
        &self,
        _rsvp: crate::ReservationId,
        _note: String,
    ) -> Result<abi::Reservation, ReservationError> {
        todo!()
    }

    async fn delete(&self, _rsvp: crate::ReservationId) -> Result<(), ReservationError> {
        todo!()
    }

    async fn get(&self, _rsvp: crate::ReservationId) -> Result<abi::Reservation, ReservationError> {
        todo!()
    }

    async fn query(
        &self,
        _query: abi::ReservationQuery,
    ) -> Result<Vec<abi::Reservation>, ReservationError> {
        todo!()
    }
}

mod error;
mod manager;

use async_trait::async_trait;
use sqlx::PgPool;

pub use error::ReservationError;

pub type ReservationId = String;
pub type ResourceId = String;
pub type UserId = String;

pub struct ReservationManager {
    pub pool: PgPool,
}

#[async_trait]
pub trait Rsvp {
    /// make a reservation
    async fn reserve(&self, rsvp: abi::Reservation) -> Result<abi::Reservation, ReservationError>;
    /// change reservation status
    async fn change_status(
        &self,
        rsvp: abi::Reservation,
    ) -> Result<abi::Reservation, ReservationError>;
    /// update note
    async fn update_note(
        &self,
        rsvp: ReservationId,
        note: String,
    ) -> Result<abi::Reservation, ReservationError>;
    /// delete reservation
    async fn delete(&self, rsvp: ReservationId) -> Result<(), ReservationError>;
    /// get reservation by id
    async fn get(&self, rsvp: ReservationId) -> Result<abi::Reservation, ReservationError>;
    /// query reservations
    async fn query(
        &self,
        query: abi::ReservationQuery,
    ) -> Result<Vec<abi::Reservation>, ReservationError>;
}

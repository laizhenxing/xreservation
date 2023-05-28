mod manager;

use abi::Error;
use async_trait::async_trait;
use sqlx::PgPool;

pub struct ReservationManager {
    pub pool: PgPool,
}

#[async_trait]
pub trait Rsvp {
    /// make a reservation
    async fn reserve(&self, rsvp: abi::Reservation) -> Result<abi::Reservation, Error>;
    /// change reservation status
    async fn change_status(&self, rsvp: abi::ReservationId) -> Result<abi::Reservation, Error>;
    /// update note
    async fn update_note(
        &self,
        rsvp: abi::ReservationId,
        note: String,
    ) -> Result<abi::Reservation, Error>;
    /// delete reservation
    async fn delete(&self, rsvp: abi::ReservationId) -> Result<(), Error>;
    /// get reservation by id
    async fn get(&self, rsvp: abi::ReservationId) -> Result<abi::Reservation, Error>;
    /// query reservations
    async fn query(&self, query: abi::ReservationQuery) -> Result<Vec<abi::Reservation>, Error>;
}

impl ReservationManager {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

mod manager;

use abi::Error;
use async_trait::async_trait;
use sqlx::PgPool;
use tokio::sync::mpsc;

pub struct ReservationManager {
    pub pool: PgPool,
}

#[async_trait]
pub trait Rsvp {
    /// make a reservation
    async fn reserve(&self, rsvp: abi::Reservation) -> Result<abi::Reservation, Error>;
    /// change reservation status
    async fn change_status(&self, id: abi::ReservationId) -> Result<abi::Reservation, Error>;
    /// update note
    async fn update_note(
        &self,
        id: abi::ReservationId,
        note: String,
    ) -> Result<abi::Reservation, Error>;
    /// delete reservation
    async fn delete(&self, id: abi::ReservationId) -> Result<abi::Reservation, Error>;
    /// get reservation by id
    async fn get(&self, id: abi::ReservationId) -> Result<abi::Reservation, Error>;
    /// query reservations
    async fn query(
        &self,
        query: abi::ReservationQuery,
    ) -> mpsc::Receiver<Result<abi::Reservation, Error>>;
    /// filter reservations
    async fn filter(
        &self,
        filter: abi::ReservationFilter,
    ) -> Result<(abi::FilterPager, Vec<abi::Reservation>), Error>;
}

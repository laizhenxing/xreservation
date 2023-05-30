mod conflict;

use sqlx::postgres::PgDatabaseError;

pub use conflict::*;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("database error")]
    DbError(sqlx::Error),

    #[error("invalid start/end time")]
    InvalidTimespan,

    #[error("Invalid user id: {0}")]
    InvalidUserId(String),

    #[error("Invalid reservation id: {0}")]
    InvalidReservationId(i64),

    #[error("Invalid resource id: {0}")]
    InvalidResourceId(String),

    #[error("Not found the reservation by given condition")]
    NotFound,

    #[error("Conflict reservation")]
    ConflictReservation(ReservationConflictInfo),

    #[error("unknown data store error")]
    Unknown,
}

impl From<sqlx::Error> for Error {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::Database(err) => {
                let e: &PgDatabaseError = err.downcast_ref();
                match (e.code(), e.schema(), e.table()) {
                    ("23P01", Some("rsvp"), Some("reservations")) => {
                        Error::ConflictReservation(e.detail().unwrap().parse().unwrap())
                    }
                    _ => Error::DbError(sqlx::Error::Database(err)),
                }
            }
            sqlx::Error::RowNotFound => Self::NotFound,
            _ => Self::DbError(err),
        }
    }
}

impl PartialEq for Error {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::DbError(_), Self::DbError(_)) => true,
            (Self::InvalidTimespan, Self::InvalidTimespan) => true,
            (Self::InvalidUserId(a), Self::InvalidUserId(b)) => a == b,
            (Self::InvalidReservationId(a), Self::InvalidReservationId(b)) => a == b,
            (Self::InvalidResourceId(a), Self::InvalidResourceId(b)) => a == b,
            (Self::ConflictReservation(a), Self::ConflictReservation(b)) => a == b,
            (Self::NotFound, Self::NotFound) => true,
            (Self::Unknown, Self::Unknown) => true,
            _ => false,
        }
    }
}

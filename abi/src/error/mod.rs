mod conflict;

use sqlx::postgres::PgDatabaseError;

pub use conflict::*;
use tonic::Status;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("database error")]
    DbError(sqlx::Error),

    #[error("config read error")]
    ConfigReadError,

    #[error("config parse error")]
    ConfigParseError,

    #[error("invalid start/end time")]
    InvalidTimespan,

    #[error("Invalid user id: {0}")]
    InvalidUserId(String),

    #[error("Invalid reservation id: {0}")]
    InvalidReservationId(i64),

    #[error("Invalid resource id: {0}")]
    InvalidResourceId(String),

    #[error("missing argument: {0}")]
    MissingArgument(String),

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
            (Self::ConfigReadError, Self::ConfigReadError) => true,
            (Self::ConfigParseError, Self::ConfigParseError) => true,
            (Self::MissingArgument(a), Self::MissingArgument(b)) => a == b,
            _ => false,
        }
    }
}

impl From<Error> for tonic::Status {
    fn from(err: Error) -> Self {
        match err {
            Error::DbError(_) | Error::ConfigReadError | Error::ConfigParseError => {
                Status::internal(err.to_string())
            }
            Error::InvalidTimespan
            | Error::InvalidUserId(_)
            | Error::InvalidReservationId(_)
            | Error::InvalidResourceId(_)
            | Error::MissingArgument(_) => Status::invalid_argument(err.to_string()),
            Error::NotFound => Status::not_found("not found the reservation by given condition"),
            Error::ConflictReservation(info) => {
                Status::already_exists(format!("Conflict reservation: {:?}", info))
            }
            Error::Unknown => Status::internal("unknown error"),
        }
    }
}

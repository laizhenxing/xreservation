use thiserror::Error;

#[derive(Error, Debug)]
pub enum ReservationError {
    // #[error("data store disconnected")]
    // Disconnect(#[from] io::Error),
    // #[error("the data for key `{0}` is not available")]
    // Redaction(String),
    // #[error("invalid header (expected {expected:?}, found {found:?})")]
    // InvalidHeader {
    //     expected: String,
    //     found: String,
    // },
    /// deal with sqlx::Error
    #[error("database error")]
    DbError(#[from] sqlx::Error),
    #[error("invalid start/end time")]
    InvalidTimespan,
    #[error("unknown data store error")]
    Unknown,
}

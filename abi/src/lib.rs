mod config;
mod error;
mod pager;
mod pb;
mod types;
mod utils;

pub use config::*;
pub use error::{Error, ReservationConflict, ReservationConflictInfo, ReservationWindow};
pub use pager::*;
pub use pb::*;
pub use utils::*;

/// 为了方便, 将一些类型定义在这里
/// 别名的好处在于当此类型发生改变时, 只需要修改此处即可
pub type ReservationId = i64;
pub type ResourceId = String;
pub type UserId = String;

pub trait Validator {
    fn validate(&self) -> Result<(), Error>;
}

pub trait Normalizer: Validator {
    fn normalize(&mut self) -> Result<(), Error> {
        self.validate()?;
        self.do_normalize();
        Ok(())
    }

    fn do_normalize(&mut self);
}

pub trait ToSql {
    fn to_sql(&self) -> String;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "reservation_status", rename_all = "lowercase")]
pub enum RsvpStatus {
    Unknown,
    Pending,
    Confirmed,
    Blocked,
}

impl Validator for ReservationId {
    fn validate(&self) -> Result<(), Error> {
        if *self <= 0 {
            return Err(Error::InvalidReservationId(*self));
        }

        Ok(())
    }
}

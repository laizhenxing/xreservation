/// 告诉编译器,忽略 camel_case_types 的警告
#[allow(clippy::all, non_camel_case_types)]
mod reservation;

pub use reservation::*;

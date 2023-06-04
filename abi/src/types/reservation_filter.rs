use std::collections::VecDeque;

use crate::{
    Error, FilterPager, Id, Normalizer, PageInfo, Pager, Paginator, ReservationFilter,
    ReservationStatus, ToSql, Validator,
};

impl ReservationFilter {
    pub fn get_status(&self) -> ReservationStatus {
        ReservationStatus::from_i32(self.status).unwrap()
    }

    pub fn get_cursor(&self) -> i64 {
        self.cursor.unwrap_or(if self.desc { i64::MAX } else { 0 })
    }

    pub fn get_pager<T: Id>(&self, data: &mut VecDeque<T>) -> FilterPager {
        let page_info = self.get_page_info();
        let pager = page_info.get_pager(data);
        pager.into()
    }

    pub fn get_page_info(&self) -> PageInfo {
        PageInfo {
            cursor: self.cursor,
            page_size: self.page_size,
            desc: self.desc,
        }
    }
}

impl Validator for ReservationFilter {
    fn validate(&self) -> Result<(), Error> {
        if self.page_size < 10 || self.page_size > 100 {
            return Err(Error::InvalidPageSize(self.page_size));
        }

        if let Some(cursor) = self.cursor {
            if cursor < 0 {
                return Err(Error::InvalidCursor(cursor));
            }
        }

        ReservationStatus::from_i32(self.status).ok_or(Error::InvalidStatus(self.status))?;

        Ok(())
    }
}

impl Normalizer for ReservationFilter {
    fn do_normalize(&mut self) {
        if self.status == ReservationStatus::Unknown as i32 {
            self.status = ReservationStatus::Pending as i32;
        }
    }
}

impl ToSql for ReservationFilter {
    fn to_sql(&self) -> String {
        let middle_plus = if self.cursor.is_some() { 1 } else { 0 };
        let limit = self.page_size + 1 + middle_plus;

        let cursor_condition = if self.desc {
            format!("id <= {}", self.get_cursor())
        } else {
            format!("id >= {}", self.get_cursor())
        };
        let status = self.get_status();

        let user_resource_condition = match (self.user_id.is_empty(), self.resource_id.is_empty()) {
            (true, true) => "TRUE".into(),
            (true, false) => format!("resource_id = '{}'", self.resource_id),
            (false, true) => format!("user_id = '{}'", self.user_id),
            (false, false) => format!(
                "user_id = '{}' AND resource_id = '{}'",
                self.user_id, self.resource_id
            ),
        };

        let order = if self.desc { "DESC" } else { "ASC" };

        format!(
            "SELECT * FROM rsvp.reservations WHERE {} AND status = '{}'::rsvp.reservation_status AND {} ORDER BY id {} LIMIT {}",
            user_resource_condition, status, cursor_condition, order, limit
        )
    }
}

impl From<Pager> for FilterPager {
    fn from(pager: Pager) -> Self {
        Self {
            prev: pager.prev,
            next: pager.next,
            total: pager.total,
        }
    }
}

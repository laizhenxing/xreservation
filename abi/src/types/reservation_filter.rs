use crate::{
    Error, FilterPager, Id, Normalizer, PageInfo, Pager, Paginator, ReservationFilter,
    ReservationStatus, ToSql, Validator,
};
use std::collections::VecDeque;

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

#[cfg(test)]
pub mod pager_test_utils {
    use crate::pager::Id;
    use std::collections::VecDeque;

    pub struct TestId(i64);

    impl Id for TestId {
        fn id(&self) -> i64 {
            self.0
        }
    }

    pub fn generate_test_ids(start: i64, end: i64) -> VecDeque<TestId> {
        (start..=end).map(TestId).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ReservationFilterBuilder;

    #[test]
    fn filter_with_wrong_page_size_should_fail() {
        let filter = ReservationFilterBuilder::default()
            .page_size(-1)
            .build()
            .unwrap();
        let err = filter.validate().unwrap_err();
        assert_eq!(err, Error::InvalidPageSize(-1));

        let filter = ReservationFilterBuilder::default()
            .page_size(101)
            .build()
            .unwrap();
        let err = filter.validate().unwrap_err();
        assert_eq!(err, Error::InvalidPageSize(101));

        let filter = ReservationFilterBuilder::default()
            .page_size(5)
            .build()
            .unwrap();
        let err = filter.validate().unwrap_err();
        assert_eq!(err, Error::InvalidPageSize(5));
    }

    #[test]
    fn filter_with_right_page_size_should_work() {
        let filter = ReservationFilterBuilder::default()
            .page_size(10)
            .build()
            .unwrap();
        assert!(filter.validate().is_ok());

        let filter = ReservationFilterBuilder::default()
            .page_size(100)
            .build()
            .unwrap();
        assert!(filter.validate().is_ok());

        let filter = ReservationFilterBuilder::default()
            .page_size(50)
            .build()
            .unwrap();
        assert!(filter.validate().is_ok());
    }

    #[test]
    fn filter_with_wrong_cursor_should_fail() {
        let filter = ReservationFilterBuilder::default()
            .cursor(-1)
            .build()
            .unwrap();
        let err = filter.validate().unwrap_err();
        assert_eq!(err, Error::InvalidCursor(-1));
    }

    #[test]
    fn filter_with_right_cursor_should_work() {
        let filter = ReservationFilterBuilder::default()
            .cursor(0)
            .build()
            .unwrap();
        assert!(filter.validate().is_ok());

        let filter = ReservationFilterBuilder::default()
            .cursor(1)
            .build()
            .unwrap();
        assert!(filter.validate().is_ok());

        let filter = ReservationFilterBuilder::default()
            .cursor(100)
            .build()
            .unwrap();
        assert!(filter.validate().is_ok());
    }

    #[test]
    fn get_pager_should_work() {
        let filter = ReservationFilterBuilder::default().build().unwrap();
        let page_info = filter.get_page_info();
        assert!(page_info.cursor.is_none());
        assert!(!page_info.desc);
        assert_eq!(page_info.page_size, 10);

        let mut data = pager_test_utils::generate_test_ids(1, 10);
        let pager = page_info.get_pager(&mut data);
        assert!(pager.prev.is_none());
        assert!(pager.next.is_none());

        let mut data = pager_test_utils::generate_test_ids(1, 11);
        let pager = page_info.get_pager(&mut data);
        assert!(pager.prev.is_none());
        assert_eq!(pager.next, Some(11));

        let filter = ReservationFilterBuilder::default()
            .cursor(5)
            .build()
            .unwrap();
        let page_info = filter.get_page_info();
        let mut data = pager_test_utils::generate_test_ids(5, 10);
        let pager = page_info.get_pager(&mut data);
        assert_eq!(pager.prev, Some(5));
        assert!(pager.next.is_none());

        let mut data = pager_test_utils::generate_test_ids(5, 15);
        let pager = page_info.get_pager(&mut data);
        assert_eq!(pager.prev, Some(5));
        assert_eq!(pager.next, Some(15));
    }

    #[test]
    fn filter_to_sql_should_work() {
        let mut filter = ReservationFilterBuilder::default()
            .cursor(5)
            .page_size(13)
            .desc(true)
            .build()
            .unwrap();

        filter.normalize().unwrap();

        let sql = filter.to_sql();
        assert_eq!(
            sql,
            "SELECT * FROM rsvp.reservations WHERE TRUE AND status = 'pending'::rsvp.reservation_status AND id <= 5 ORDER BY id DESC LIMIT 15"
        );

        let mut filter = ReservationFilterBuilder::default()
            .cursor(2)
            .user_id("test-uid-1")
            .page_size(12)
            .desc(false)
            .build()
            .unwrap();
        filter.normalize().unwrap();

        let sql = filter.to_sql();
        assert_eq!(
            sql,
            "SELECT * FROM rsvp.reservations WHERE user_id = 'test-uid-1' AND status = 'pending'::rsvp.reservation_status AND id >= 2 ORDER BY id ASC LIMIT 14"
        );

        let mut filter = ReservationFilterBuilder::default()
            .user_id("test-uid-1")
            .page_size(12)
            .desc(true)
            .build()
            .unwrap();
        filter.normalize().unwrap();

        let sql = filter.to_sql();
        assert_eq!(
            sql,
            "SELECT * FROM rsvp.reservations WHERE user_id = 'test-uid-1' AND status = 'pending'::rsvp.reservation_status AND id <= 9223372036854775807 ORDER BY id DESC LIMIT 13"
        );

        let mut filter = ReservationFilterBuilder::default()
            .user_id("test-uid-1")
            .page_size(12)
            .desc(false)
            .build()
            .unwrap();
        filter.normalize().unwrap();
        assert_eq!(
            filter.to_sql(),
            "SELECT * FROM rsvp.reservations WHERE user_id = 'test-uid-1' AND status = 'pending'::rsvp.reservation_status AND id >= 0 ORDER BY id ASC LIMIT 13"
        );
    }
}

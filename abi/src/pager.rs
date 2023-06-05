use std::collections::VecDeque;

pub struct PageInfo {
    pub cursor: Option<i64>,
    pub page_size: i64,
    pub desc: bool,
}

pub struct Pager {
    pub prev: Option<i64>,
    pub next: Option<i64>,
    pub total: Option<i64>,
}

pub trait Paginator: Sized {
    fn get_pager<T: Id>(&self, data: &mut VecDeque<T>) -> Pager;
    fn next_page(&self, pager: &Pager) -> Option<Self>;
    fn prev_page(&self, pager: &Pager) -> Option<Self>;
}

pub trait Id {
    fn id(&self) -> i64;
}

impl Paginator for PageInfo {
    fn get_pager<T: Id>(&self, data: &mut VecDeque<T>) -> Pager {
        let has_prev = self.cursor.is_some();
        let prev = if has_prev {
            data.front().map(|x| x.id())
        } else {
            None
        };

        let has_next = data.len() as i64 > self.page_size;
        let next = if has_next {
            data.back().map(|x| x.id())
        } else {
            None
        };

        Pager {
            prev,
            next,
            total: None,
        }
    }

    fn next_page(&self, pager: &Pager) -> Option<Self> {
        if pager.next.is_some() {
            Some(Self {
                cursor: pager.next,
                page_size: self.page_size,
                desc: self.desc,
            })
        } else {
            None
        }
    }

    fn prev_page(&self, pager: &Pager) -> Option<Self> {
        if pager.prev.is_some() {
            Some(Self {
                cursor: pager.prev,
                page_size: self.page_size,
                desc: self.desc,
            })
        } else {
            None
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

    #[test]
    fn paginator_should_work() {
        // first page
        let page_info = PageInfo {
            cursor: None,
            page_size: 10,
            desc: false,
        };
        let mut data = pager_test_utils::generate_test_ids(1, 11);

        let pager = page_info.get_pager(&mut data);
        assert_eq!(pager.prev, None);
        assert_eq!(pager.next, Some(11));

        {
            let prev_page = page_info.prev_page(&pager);
            assert!(prev_page.is_none());
        }

        // second page
        let page = page_info.next_page(&pager).unwrap();
        let mut data = pager_test_utils::generate_test_ids(11, 21);
        let pager = page.get_pager(&mut data);
        assert_eq!(pager.prev, Some(11));
        assert_eq!(pager.next, Some(21));

        {
            let prev_page = page.prev_page(&pager);
            assert_eq!(prev_page.unwrap().cursor, Some(11));
        }

        // third page
        let page = page_info.next_page(&pager).unwrap();
        let mut data = pager_test_utils::generate_test_ids(21, 30);
        let pager = page.get_pager(&mut data);
        assert_eq!(pager.prev, Some(21));
        assert_eq!(pager.next, None);

        {
            let prev_page = page.prev_page(&pager);
            assert_eq!(prev_page.unwrap().cursor, Some(21));
        }
    }
}

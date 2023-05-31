use crate::{ReservationManager, Rsvp};
use abi::{
    DbConfig, Error, FilterPager, Reservation, ReservationFilter, ReservationId, ReservationQuery,
    ReservationStatus, Validator,
};

use async_trait::async_trait;
use sqlx::{pool::PoolOptions, PgPool, Row};

impl ReservationManager {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn from_config(config: &DbConfig) -> Result<Self, Error> {
        let pool = PoolOptions::new()
            .max_connections(config.max_connections)
            .connect(&config.url())
            .await?;
        Ok(Self::new(pool))
    }
}

#[async_trait]
impl Rsvp for ReservationManager {
    async fn reserve(&self, mut rsvp: Reservation) -> Result<abi::Reservation, Error> {
        rsvp.validate()?;

        let timespan = rsvp.get_timespan();

        let status = ReservationStatus::from_i32(rsvp.status).unwrap_or(ReservationStatus::Pending);

        // stauts 默认类型 text, 这里需要转换成 rsvp.reservation_status
        let sql = "INSERT INTO rsvp.reservations (user_id, resource_id, timespan, note, status)
            VALUES ($1, $2, $3, $4, $5::rsvp.reservation_status) RETURNING id";
        let id: i64 = sqlx::query(sql)
            .bind(rsvp.user_id.clone())
            .bind(rsvp.resource_id.clone())
            .bind(timespan)
            .bind(rsvp.note.clone())
            .bind(status.to_string())
            .fetch_one(&self.pool)
            .await?
            .get(0);

        rsvp.id = id;

        Ok(rsvp)
    }

    async fn change_status(&self, id: ReservationId) -> Result<Reservation, Error> {
        id.validate()?;

        let sql = "UPDATE rsvp.reservations SET status = 'confirmed'::rsvp.reservation_status WHERE id = $1 AND status = 'pending' RETURNING *";
        let rsvp = sqlx::query_as(sql).bind(id).fetch_one(&self.pool).await?;

        Ok(rsvp)
    }

    async fn update_note(&self, id: ReservationId, note: String) -> Result<Reservation, Error> {
        id.validate()?;

        let sql = "UPDATE rsvp.reservations SET note = $1 WHERE id = $2 RETURNING *";
        let rsvp = sqlx::query_as(sql)
            .bind(note)
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
        Ok(rsvp)
    }

    async fn delete(&self, id: ReservationId) -> Result<(), Error> {
        id.validate()?;

        let sql = "DELETE FROM rsvp.reservations WHERE id = $1";

        sqlx::query(sql).bind(id).execute(&self.pool).await?;
        Ok(())
    }

    async fn get(&self, id: ReservationId) -> Result<Reservation, Error> {
        id.validate()?;

        let sql = "SELECT * FROM rsvp.reservations WHERE id = $1";
        let rsvp = sqlx::query_as(sql).bind(id).fetch_one(&self.pool).await?;
        Ok(rsvp)
    }

    async fn query(&self, query: ReservationQuery) -> Result<Vec<Reservation>, Error> {
        let uid = str_to_option(&query.user_id);
        let rid = str_to_option(&query.resource_id);
        let timespan = query.get_timespan();
        let status =
            ReservationStatus::from_i32(query.status).unwrap_or(ReservationStatus::Pending);

        let rsvps = sqlx::query_as(
            "SELECT * FROM rsvp.query($1, $2, $3, $4::rsvp.reservation_status, $5, $6, $7)",
        )
        .bind(uid)
        .bind(rid)
        .bind(timespan)
        .bind(status.to_string())
        .bind(query.page)
        .bind(query.desc)
        .bind(query.page_size)
        .fetch_all(&self.pool)
        .await?;

        Ok(rsvps)
    }

    /// filter reservations by user_id, resource_id, status, cursor, desc, page_size
    async fn filter(
        &self,
        filter: ReservationFilter,
    ) -> Result<(FilterPager, Vec<Reservation>), Error> {
        let uid = str_to_option(&filter.user_id);
        let rid = str_to_option(&filter.resource_id);
        let status =
            ReservationStatus::from_i32(filter.status).unwrap_or(ReservationStatus::Pending);
        // page_size must between 10 and 100
        let page_size = if filter.page_size < 10 || filter.page_size > 100 {
            10
        } else {
            filter.page_size
        };

        let rsvps: Vec<Reservation> = sqlx::query_as(
            "SELECT * FROM rsvp.filter($1, $2, $3::rsvp.reservation_status, $4, $5, $6)",
        )
        .bind(uid)
        .bind(rid)
        .bind(status.to_string())
        .bind(filter.cursor)
        .bind(filter.desc)
        .bind(page_size)
        .fetch_all(&self.pool)
        .await?;

        // TODO: incomplete FilterPager
        // if the first id is current cursor, then there is have prev, start from 1
        // if len - start > page_size, then there is have next, end at len-1
        let has_pre = !rsvps.is_empty() && rsvps[0].id == filter.cursor;
        let start = if has_pre { 1 } else { 0 };

        let has_next = (rsvps.len() - start) as i32 > page_size;
        let end = if has_next {
            rsvps.len() - 1
        } else {
            rsvps.len()
        };

        let prev = if has_pre { rsvps[start - 1].id } else { -1 };
        let next = if has_next { rsvps[end - 1].id } else { -1 };

        let result = rsvps[start..end].to_vec();

        let pager = FilterPager {
            prev,
            next,
            // TODO: get total from an efficient way instead of query all
            total: 0,
        };
        Ok((pager, result))
    }
}

fn str_to_option(s: &str) -> Option<&str> {
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use abi::ReservationFilterBuilder;
    use abi::{
        convert_to_timestamp, Error, Reservation, ReservationConflict, ReservationConflictInfo,
        ReservationQueryBuilder, ReservationWindow,
    };
    use chrono::DateTime;
    use chrono::FixedOffset;
    use prost_types::Timestamp;
    use sqlx::PgPool;

    #[sqlx_database_tester::test(pool(variable = "migrated_pool", migrations = "../migrations"))]
    async fn reserve_should_work_with_valid_window() {
        let manager = ReservationManager::new(migrated_pool.clone());

        let start: DateTime<FixedOffset> = "2023-1-1T10:10:10-0700".parse().unwrap();
        let end: DateTime<FixedOffset> = "2023-1-4T10:10:10-0700".parse().unwrap();

        let rsvp = Reservation::new(
            "test-user".to_string(),
            "test-resource".to_string(),
            start,
            end,
            "test-note".to_string(),
        );

        let rsvp = manager.reserve(rsvp).await.unwrap();
        assert_eq!(rsvp.id, 1);
    }

    #[sqlx_database_tester::test(pool(variable = "migrated_pool", migrations = "../migrations"))]
    async fn reserve_should_fail_with_invalid_window() {
        let manager = ReservationManager::new(migrated_pool.clone());

        let start: DateTime<FixedOffset> = "2023-1-1T10:10:10-0700".parse().unwrap();
        let end: DateTime<FixedOffset> = "2022-1-1T10:10:10-0700".parse().unwrap();

        let rsvp = Reservation::new("test-user", "test-resource", start, end, "test-note");

        let err = manager.reserve(rsvp).await.unwrap_err();
        assert!(matches!(err, Error::InvalidTimespan));
    }

    #[sqlx_database_tester::test(pool(variable = "migrated_pool", migrations = "../migrations"))]
    async fn reserve_conflic_reservation_should_reject() {
        let (_rvsp1, manager) = make_reservation(
            migrated_pool.clone(),
            "test-user",
            "test-resource",
            "2023-1-1T10:10:10-0700",
            "2023-1-4T10:10:10-0700",
            "test-note",
        )
        .await;

        let rvsp2 = Reservation::new(
            "test-user",
            "test-resource",
            "2023-1-2T10:10:10-0700".parse().unwrap(),
            "2023-1-5T10:10:10-0700".parse().unwrap(),
            "test-note",
        );
        //let _rvsp1 = manager.reserve(rvsp1).await.unwrap();
        let err = manager.reserve(rvsp2).await.unwrap_err();

        let info = ReservationConflictInfo::Parsed(ReservationConflict {
            old: ReservationWindow {
                rid: "test-resource".to_string(),
                start: "2023-1-1T10:10:10-0700".parse().unwrap(),
                end: "2023-01-04T10:10:10-0700".parse().unwrap(),
            },
            new: ReservationWindow {
                rid: "test-resource".to_string(),
                start: "2023-1-2T10:10:10-0700".parse().unwrap(),
                end: "2023-1-5T10:10:10-0700".parse().unwrap(),
            },
        });

        assert_eq!(err, Error::ConflictReservation(info));
    }

    #[sqlx_database_tester::test(pool(variable = "migrated_pool", migrations = "../migrations"))]
    async fn reserve_with_empty_start_timestamp_should_fail() {
        let manager = ReservationManager::new(migrated_pool.clone());

        let rsvp = Reservation {
            user_id: "test-user".to_string(),
            resource_id: "test-resource".to_string(),
            end: Some(convert_to_timestamp(
                &"2023-1-2T10:10:10-0700".parse().unwrap(),
            )),
            ..Default::default()
        };

        let err = manager.reserve(rsvp).await.unwrap_err();
        assert!(matches!(err, Error::InvalidTimespan));
    }

    #[sqlx_database_tester::test(pool(variable = "migrated_pool", migrations = "../migrations"))]
    async fn reserve_with_empty_end_timestamp_should_fail() {
        let manager = ReservationManager::new(migrated_pool.clone());

        let rsvp = Reservation {
            user_id: "test-user".to_string(),
            resource_id: "test-resource".to_string(),
            start: Some(convert_to_timestamp(
                &"2023-1-2T10:10:10-0700".parse().unwrap(),
            )),
            ..Default::default()
        };

        let err = manager.reserve(rsvp).await.unwrap_err();
        assert!(matches!(err, Error::InvalidTimespan));
    }

    #[sqlx_database_tester::test(pool(variable = "migrated_pool", migrations = "../migrations"))]
    async fn reserver_with_empty_user_id_should_fail() {
        let manager = ReservationManager::new(migrated_pool.clone());

        let rsvp = Reservation::default();

        let err = manager.reserve(rsvp).await.unwrap_err();
        assert!(matches!(err, Error::InvalidUserId(..)));
    }

    #[sqlx_database_tester::test(pool(variable = "migrated_pool", migrations = "../migrations"))]
    async fn reserver_with_empty_resource_id_should_fail() {
        let manager = ReservationManager::new(migrated_pool.clone());

        let rsvp = Reservation {
            user_id: "test-user".to_string(),
            ..Default::default()
        };

        let err = manager.reserve(rsvp).await.unwrap_err();
        assert!(matches!(err, Error::InvalidResourceId(..)));
    }

    #[sqlx_database_tester::test(pool(variable = "migrated_pool", migrations = "../migrations"))]
    async fn change_status_should_work() {
        let (rsvp, manager) = make_reservation(
            migrated_pool.clone(),
            "test-user",
            "test-resource",
            "2023-1-1T10:10:10-0700",
            "2023-1-4T10:10:10-0700",
            "test-note",
        )
        .await;

        let rsvp = manager.change_status(rsvp.id).await.unwrap();
        assert_eq!(rsvp.status, ReservationStatus::Confirmed as i32);
    }

    #[sqlx_database_tester::test(pool(variable = "migrated_pool", migrations = "../migrations"))]
    async fn change_status_should_fail_with_invalid_id() {
        let manager = ReservationManager::new(migrated_pool.clone());

        let err = manager.change_status(0).await.unwrap_err();
        assert_eq!(err, Error::InvalidReservationId(0));

        let err = manager.change_status(5).await.unwrap_err();
        assert_eq!(err, Error::NotFound);
    }

    #[sqlx_database_tester::test(pool(variable = "migrated_pool", migrations = "../migrations"))]
    async fn update_note_should_work() {
        let (rsvp, manager) = make_reservation(
            migrated_pool.clone(),
            "test-user",
            "test-resource",
            "2023-1-1T10:10:10-0700",
            "2023-1-4T10:10:10-0700",
            "test-note",
        )
        .await;

        let rsvp = manager
            .update_note(rsvp.id, "new-note".to_string())
            .await
            .unwrap();
        assert_eq!(rsvp.note, "new-note".to_string());
    }

    #[sqlx_database_tester::test(pool(variable = "migrated_pool", migrations = "../migrations"))]
    async fn get_reservation_should_work() {
        let (rsvp, manager) = make_reservation(
            migrated_pool.clone(),
            "test-user",
            "test-resource",
            "2023-1-1T10:10:10-0700",
            "2023-1-4T10:10:10-0700",
            "test-note",
        )
        .await;

        let rsvp = manager.get(rsvp.id).await.unwrap();
        assert_eq!(rsvp.id, rsvp.id);
        assert_eq!(rsvp.user_id, "test-user".to_string());
        assert_eq!(rsvp.resource_id, "test-resource".to_string());
        assert_eq!(
            rsvp.start,
            Some(convert_to_timestamp(
                &"2023-1-1T10:10:10-0700".parse().unwrap()
            ))
        );
        assert_eq!(
            rsvp.end,
            Some(convert_to_timestamp(
                &"2023-1-4T10:10:10-0700".parse().unwrap()
            ))
        );
        assert_eq!(rsvp.note, "test-note".to_string());
        assert_eq!(rsvp.status, ReservationStatus::Pending as i32);
    }

    #[sqlx_database_tester::test(pool(variable = "migrated_pool", migrations = "../migrations"))]
    async fn delete_reservation_should_work() {
        let (rsvp, manager) = make_reservation(
            migrated_pool.clone(),
            "test-user",
            "test-resource",
            "2023-1-1T10:10:10-0700",
            "2023-1-4T10:10:10-0700",
            "test-note",
        )
        .await;

        manager.delete(rsvp.id).await.unwrap();

        let err = manager.get(rsvp.id).await.unwrap_err();
        assert_eq!(err, Error::NotFound);
    }

    #[sqlx_database_tester::test(pool(variable = "migrated_pool", migrations = "../migrations"))]
    async fn query_reservations_should_work() {
        let (rsvp, manager) = make_reservation(
            migrated_pool.clone(),
            "test-user",
            "test-resource",
            "2023-1-1T10:10:10-0700",
            "2023-1-4T10:10:10-0700",
            "test-note",
        )
        .await;

        let start = "2023-01-01T10:10:10-0700".parse::<Timestamp>().unwrap();
        let end = "2023-01-04T10:10:10-0700".parse::<Timestamp>().unwrap();

        // 遇到缺省值问题, 未设置 page, page_size 时, 默认为 0, 查不到数据
        let query = ReservationQueryBuilder::default()
            .user_id("test-user")
            //.resource_id("test-resource")
            .start(start)
            .end(end)
            .status(ReservationStatus::Pending as i32)
            .build()
            .unwrap();

        let rsvps = manager.query(query.clone()).await.unwrap();

        assert_eq!(rsvps.len(), 1);
        assert_eq!(rsvps[0], rsvp);

        // 将查到的数据删除,再查询,查不到数据
        manager.delete(rsvp.id).await.unwrap();
        let rsvps = manager.query(query).await.unwrap();
        assert_eq!(rsvps.len(), 0);
    }

    #[sqlx_database_tester::test(pool(variable = "migrated_pool", migrations = "../migrations"))]
    async fn filter_reservation_should_work() {
        let manager = ReservationManager::new(migrated_pool.clone());
        let rsvps = make_reservations(migrated_pool.clone()).await;

        let filter = ReservationFilterBuilder::default()
            .user_id("test-user")
            .resource_id("test-resource")
            .status(ReservationStatus::Pending as i32)
            .cursor(0)
            .desc(false)
            .build()
            .unwrap();

        let (_pager, res) = manager.filter(filter).await.unwrap();
        assert_eq!(rsvps.len(), res.len());

        let filter = ReservationFilterBuilder::default()
            .user_id("test-user")
            .resource_id("test-resource")
            .status(ReservationStatus::Pending as i32)
            .cursor(3)
            .desc(false)
            .build()
            .unwrap();
        let (_pager, res) = manager.filter(filter).await.unwrap();
        assert_eq!(rsvps.len() - 3, res.len());

        let filter = ReservationFilterBuilder::default()
            .user_id("test-user")
            .resource_id("test-resource")
            .status(ReservationStatus::Pending as i32)
            .cursor(3)
            .desc(true)
            .build()
            .unwrap();
        let (_pager, res) = manager.filter(filter).await.unwrap();
        assert_eq!(2, res.len());
    }

    #[sqlx_database_tester::test(pool(variable = "migrated_pool", migrations = "../migrations"))]
    async fn filter_reservation_with_null_cursor_should_work() {
        let manager = ReservationManager::new(migrated_pool.clone());
        let rsvps = make_reservations(migrated_pool.clone()).await;
        let filter_asc = ReservationFilterBuilder::default()
            .user_id("test-user")
            .resource_id("test-resource")
            .status(ReservationStatus::Pending as i32)
            .desc(false)
            .build()
            .unwrap();

        // if cursor is null, it should be treated as 0
        let filter_desc = ReservationFilterBuilder::default()
            .user_id("test-user")
            .resource_id("test-resource")
            .status(ReservationStatus::Pending as i32)
            .desc(true)
            .build()
            .unwrap();
        let (_asc_pager, res_asc) = manager.filter(filter_asc).await.unwrap();
        let (_desc_pager, res_desc) = manager.filter(filter_desc).await.unwrap();

        assert_eq!(rsvps.len(), res_asc.len());
        assert_eq!(res_desc.len(), 0);
    }

    #[allow(dead_code)]
    async fn make_test_reservation(migrated_pool: PgPool) -> (Reservation, ReservationManager) {
        make_reservation(
            migrated_pool.clone(),
            "test-user",
            "test-resource",
            "2023-1-1T10:10:10-0700",
            "2023-1-4T10:10:10-0700",
            "test-note",
        )
        .await
    }

    async fn make_reservations(migrated_pool: PgPool) -> Vec<Reservation> {
        let mut rsvps = Vec::new();
        let ts = vec![
            ("2023-01-01T10:10:10-0800", "2023-01-02T10:10:10-0800"),
            ("2023-01-03T10:10:10-0800", "2023-01-04T10:10:10-0800"),
            ("2023-01-05T10:10:10-0800", "2023-01-06T10:10:10-0800"),
            ("2023-01-07T10:10:10-0800", "2023-01-08T10:10:10-0800"),
            ("2023-01-09T10:10:10-0800", "2023-01-10T10:10:10-0800"),
            ("2023-01-11T10:10:10-0800", "2023-01-12T10:10:10-0800"),
            ("2023-01-13T10:10:10-0800", "2023-01-14T10:10:10-0800"),
            ("2023-01-15T10:10:10-0800", "2023-01-16T10:10:10-0800"),
            ("2023-01-17T10:10:10-0800", "2023-01-18T10:10:10-0800"),
            ("2023-01-19T10:10:10-0800", "2023-01-20T10:10:10-0800"),
        ];
        for (i, (start, end)) in ts.iter().enumerate() {
            let (rsvp, _) = make_reservation(
                migrated_pool.clone(),
                "test-user",
                "test-resource",
                start,
                end,
                &format!("test-note-{}", i),
            )
            .await;
            rsvps.push(rsvp);
        }
        rsvps
    }

    async fn make_reservation(
        pool: PgPool,
        uid: &str,
        rid: &str,
        start: &str,
        end: &str,
        note: &str,
    ) -> (Reservation, ReservationManager) {
        let manager = ReservationManager::new(pool.clone());

        let rsvp = Reservation::new(uid, rid, start.parse().unwrap(), end.parse().unwrap(), note);

        let rsvp = manager.reserve(rsvp).await.unwrap();

        (rsvp, manager)
    }
}

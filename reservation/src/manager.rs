use crate::{ReservationManager, Rsvp};
use abi::{
    DbConfig, Error, FilterPager, Normalizer, Reservation, ReservationFilter, ReservationId,
    ReservationQuery, ReservationStatus, ToSql, Validator,
};

use async_trait::async_trait;
use futures::stream::StreamExt;
use sqlx::{pool::PoolOptions, Either, PgPool, Row};
use tokio::sync::mpsc;
use tracing::{info, warn};

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

    async fn delete(&self, id: ReservationId) -> Result<Reservation, Error> {
        id.validate()?;

        let sql = "DELETE FROM rsvp.reservations WHERE id = $1 RETURNING *";
        let rsvp = sqlx::query_as(sql).bind(id).fetch_one(&self.pool).await?;

        Ok(rsvp)
    }

    async fn get(&self, id: ReservationId) -> Result<Reservation, Error> {
        id.validate()?;

        let sql = "SELECT * FROM rsvp.reservations WHERE id = $1";
        let rsvp = sqlx::query_as(sql).bind(id).fetch_one(&self.pool).await?;

        Ok(rsvp)
    }

    async fn query(&self, query: ReservationQuery) -> mpsc::Receiver<Result<Reservation, Error>> {
        let pool = self.pool.clone();

        // use channel to send query result
        let (tx, rx) = mpsc::channel(128);

        tokio::spawn(async move {
            let sql = query.to_sql();
            let mut rsvps = sqlx::query_as(&sql).fetch_many(&pool);

            // send query result to channel
            while let Some(ret) = rsvps.next().await {
                match ret {
                    Ok(Either::Left(r)) => {
                        info!("Query result: {:?}", r);
                    }
                    Ok(Either::Right(r)) => {
                        if tx.send(Ok(r)).await.is_err() {
                            // rx is dropped, so client disconnected
                            break;
                        }
                    }
                    Err(e) => {
                        warn!("Query error: {:?}", e);
                        if tx.send(Err(e.into())).await.is_err() {
                            // rx is dropped, so client disconnected
                            break;
                        }
                    }
                }
            }
        });

        rx
    }

    /// filter reservations by user_id, resource_id, status, cursor, desc, page_size
    async fn filter(
        &self,
        mut filter: ReservationFilter,
    ) -> Result<(FilterPager, Vec<Reservation>), Error> {
        filter.normalize()?;

        let sql = filter.to_sql();
        let rsvps: Vec<Reservation> = sqlx::query_as(&sql).fetch_all(&self.pool).await?;
        let mut rsvps = rsvps.into_iter().collect();

        let pager = filter.get_pager(&mut rsvps);
        Ok((pager, rsvps.into()))
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
    use xsqlx_db_tester::TestDB;

    fn get_db() -> TestDB {
        TestDB::new(
            "postgres://postgres:postgres@localhost:5432",
            "../migrations",
        )
    }

    #[tokio::test]
    async fn reserve_should_work_with_valid_window() {
        let tdb = get_db();
        let pool = tdb.get_pool().await;
        let manager = ReservationManager::new(pool.clone());

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

    #[tokio::test]
    async fn reserve_should_fail_with_invalid_window() {
        let tdb = get_db();
        let pool = tdb.get_pool().await;
        let manager = ReservationManager::new(pool.clone());

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

    #[tokio::test]
    async fn reserve_with_empty_start_timestamp_should_fail() {
        let tdb = get_db();
        let pool = tdb.get_pool().await;
        let manager = ReservationManager::new(pool.clone());

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

    #[tokio::test]
    async fn reserve_with_empty_end_timestamp_should_fail() {
        let tdb = get_db();
        let pool = tdb.get_pool().await;
        let manager = ReservationManager::new(pool.clone());

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

    #[tokio::test]
    async fn reserver_with_empty_user_id_should_fail() {
        let tdb = get_db();
        let pool = tdb.get_pool().await;
        let manager = ReservationManager::new(pool.clone());

        let rsvp = Reservation::default();

        let err = manager.reserve(rsvp).await.unwrap_err();
        assert!(matches!(err, Error::InvalidUserId(..)));
    }

    #[tokio::test]
    async fn reserver_with_empty_resource_id_should_fail() {
        let tdb = get_db();
        let pool = tdb.get_pool().await;
        let manager = ReservationManager::new(pool.clone());

        let rsvp = Reservation {
            user_id: "test-user".to_string(),
            ..Default::default()
        };

        let err = manager.reserve(rsvp).await.unwrap_err();
        assert!(matches!(err, Error::InvalidResourceId(..)));
    }

    #[tokio::test]
    async fn change_status_should_work() {
        let tdb = get_db();
        let pool = tdb.get_pool().await;
        let (rsvp, manager) = make_reservation(
            pool.clone(),
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

    #[tokio::test]
    async fn change_status_should_fail_with_invalid_id() {
        let tdb = get_db();
        let pool = tdb.get_pool().await;
        let manager = ReservationManager::new(pool.clone());

        let err = manager.change_status(0).await.unwrap_err();
        assert_eq!(err, Error::InvalidReservationId(0));

        let err = manager.change_status(5).await.unwrap_err();
        assert_eq!(err, Error::NotFound);
    }

    #[tokio::test]
    async fn update_note_should_work() {
        let tdb = get_db();
        let pool = tdb.get_pool().await;
        let (rsvp, manager) = make_reservation(
            pool.clone(),
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

    #[tokio::test]
    async fn get_reservation_should_work() {
        let tdb = get_db();
        let pool = tdb.get_pool().await;
        let (rsvp, manager) = make_reservation(
            pool.clone(),
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

    #[tokio::test]
    async fn delete_reservation_should_work() {
        let tdb = get_db();
        let pool = tdb.get_pool().await;
        let (rsvp, manager) = make_reservation(
            pool.clone(),
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

    #[tokio::test]
    async fn query_reservations_should_work() {
        let tdb = get_db();
        let pool = tdb.get_pool().await;
        let (rsvp, manager) = make_reservation(
            pool.clone(),
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

        let mut rx = manager.query(query.clone()).await;

        assert_eq!(rx.recv().await.unwrap().unwrap(), rsvp);
        assert_eq!(rx.recv().await, None);

        // 将查到的数据删除,再查询,查不到数据
        manager.delete(rsvp.id).await.unwrap();
        let mut rx = manager.query(query).await;
        assert_eq!(rx.recv().await, None);
    }

    #[tokio::test]
    async fn filter_reservation_should_work() {
        let tdb = get_db();
        let pool = tdb.get_pool().await;
        let manager = ReservationManager::new(pool.clone());
        let rsvps = make_reservations(pool.clone()).await;

        let filter = ReservationFilterBuilder::default()
            .user_id("test-user")
            .resource_id("test-resource")
            .status(ReservationStatus::Pending as i32)
            .build()
            .unwrap();

        let (pager, res) = manager.filter(filter).await.unwrap();
        assert_eq!(rsvps.len(), res.len());
        assert_eq!(pager.prev, None);
        assert_eq!(pager.next, None);

        let filter = ReservationFilterBuilder::default()
            .user_id("test-user")
            .resource_id("test-resource")
            .status(ReservationStatus::Pending as i32)
            .cursor(4)
            .desc(false)
            .build()
            .unwrap();
        let (pager, res) = manager.filter(filter).await.unwrap();
        assert_eq!(7, res.len());
        assert_eq!(pager.prev, Some(4));
        assert_eq!(pager.next, None);

        let filter = ReservationFilterBuilder::default()
            .user_id("test-user")
            .resource_id("test-resource")
            .status(ReservationStatus::Pending as i32)
            .cursor(4)
            .desc(true)
            .build()
            .unwrap();
        let (pager, res) = manager.filter(filter).await.unwrap();
        assert_eq!(4, res.len());
        assert_eq!(pager.next, None);
        assert_eq!(pager.prev, Some(4));
    }

    #[tokio::test]
    async fn filter_reservation_with_null_cursor_should_work() {
        let tdb = get_db();
        let pool = tdb.get_pool().await;
        let manager = ReservationManager::new(pool.clone());
        let _rsvps = make_reservations(pool.clone()).await;
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
        let (asc_pager, res_asc) = manager.filter(filter_asc).await.unwrap();
        let (desc_pager, res_desc) = manager.filter(filter_desc).await.unwrap();

        assert_eq!(res_asc.len(), 10);
        assert_eq!(asc_pager.prev, None);
        assert_eq!(asc_pager.next, None);

        assert_eq!(res_desc.len(), 10);
        assert_eq!(desc_pager.prev, None);
        assert_eq!(desc_pager.next, None);
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

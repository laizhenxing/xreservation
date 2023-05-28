use crate::{ReservationManager, Rsvp};
use abi::{
    convert_to_utc_time, Error, Reservation, ReservationId, ReservationQuery, ReservationStatus,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{postgres::types::PgRange, types::Uuid, Row};

#[async_trait]
impl Rsvp for ReservationManager {
    async fn reserve(&self, mut rsvp: Reservation) -> Result<abi::Reservation, Error> {
        rsvp.validate()?;

        let start = convert_to_utc_time(rsvp.start.as_ref().unwrap().clone());
        let end = convert_to_utc_time(rsvp.end.as_ref().unwrap().clone());
        let timespan: PgRange<DateTime<Utc>> = (start..end).into();

        let status = ReservationStatus::from_i32(rsvp.status).unwrap_or(ReservationStatus::Pending);

        // stauts 默认类型 text, 这里需要转换成 rsvp.reservation_status
        let sql = "INSERT INTO rsvp.reservations (user_id, resource_id, timespan, note, status)
            VALUES ($1, $2, $3, $4, $5::rsvp.reservation_status) RETURNING id";
        let id: Uuid = sqlx::query(sql)
            .bind(rsvp.user_id.clone())
            .bind(rsvp.resource_id.clone())
            .bind(timespan)
            .bind(rsvp.note.clone())
            .bind(status.to_string())
            .fetch_one(&self.pool)
            .await?
            .get(0);

        rsvp.id = id.to_string();

        Ok(rsvp)
    }

    async fn change_status(&self, id: ReservationId) -> Result<Reservation, Error> {
        let id = Uuid::parse_str(&id).map_err(|_| Error::InvalidResourceId(id.clone()))?;
        let sql = "UPDATE rsvp.reservations SET status = 'confirmed'::rsvp.reservation_status WHERE id = $1::UUID AND status = 'pending' RETURNING *";
        let rsvp = sqlx::query_as(sql).bind(id).fetch_one(&self.pool).await?;

        Ok(rsvp)
    }

    async fn update_note(&self, id: ReservationId, note: String) -> Result<Reservation, Error> {
        let id = Uuid::parse_str(&id).map_err(|_| Error::InvalidResourceId(id.clone()))?;
        let sql = "UPDATE rsvp.reservations SET note = $1 WHERE id = $2::UUID RETURNING *";
        let rsvp = sqlx::query_as(sql)
            .bind(note)
            .bind(id)
            .fetch_one(&self.pool)
            .await?;
        Ok(rsvp)
    }

    async fn delete(&self, id: ReservationId) -> Result<(), Error> {
        let id = Uuid::parse_str(&id).map_err(|_| Error::InvalidResourceId(id.clone()))?;
        let sql = "DELETE FROM rsvp.reservations WHERE id = $1::UUID";

        sqlx::query(sql).bind(id).execute(&self.pool).await?;
        Ok(())
    }

    async fn get(&self, id: ReservationId) -> Result<Reservation, Error> {
        let id = Uuid::parse_str(&id).map_err(|_| Error::InvalidResourceId(id.clone()))?;
        let sql = "SELECT * FROM rsvp.reservations WHERE id = $1::UUID";
        let rsvp = sqlx::query_as(sql).bind(id).fetch_one(&self.pool).await?;
        Ok(rsvp)
    }

    async fn query(&self, _query: ReservationQuery) -> Result<Vec<Reservation>, Error> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use abi::{
        convert_to_timestamp, Error, Reservation, ReservationConflict, ReservationConflictInfo,
        ReservationWindow,
    };
    use chrono::FixedOffset;

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
        assert!(!rsvp.id.is_empty());

        //match manager.reserve(rsvp).await {
        //    Ok(rsvp) => {
        //        assert_eq!(rsvp.user_id, "test-user");
        //        assert_eq!(rsvp.resource_id, "test-resource");
        //        assert_eq!(rsvp.note, "test-note");
        //        assert_eq!(rsvp.status, ReservationStatus::Pending as i32);
        //    }
        //    Err(e) => panic!("Failed to reserve: {:?}", e),
        //}
    }

    #[sqlx_database_tester::test(pool(variable = "migrated_pool", migrations = "../migrations"))]
    async fn reserve_should_fail_with_invalid_window() {
        let manager = ReservationManager::new(migrated_pool.clone());

        let start: DateTime<FixedOffset> = "2023-1-1T10:10:10-0700".parse().unwrap();
        let end: DateTime<FixedOffset> = "2022-1-1T10:10:10-0700".parse().unwrap();

        let rsvp = Reservation::new(
            "test-user".to_string(),
            "test-resource".to_string(),
            start,
            end,
            "test-note".to_string(),
        );

        let err = manager.reserve(rsvp).await.unwrap_err();
        assert!(matches!(err, Error::InvalidTimespan));
    }

    #[sqlx_database_tester::test(pool(variable = "migrated_pool", migrations = "../migrations"))]
    async fn reserve_conflic_reservation_should_reject() {
        let manager = ReservationManager::new(migrated_pool.clone());

        let rvsp1 = make_a_reservation();

        let rvsp2 = Reservation::new(
            "test-user".to_string(),
            "test-resource".to_string(),
            "2023-1-2T10:10:10-0700".parse().unwrap(),
            "2023-1-5T10:10:10-0700".parse().unwrap(),
            "test-note".to_string(),
        );
        let _rvsp1 = manager.reserve(rvsp1).await.unwrap();
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
        let manager = ReservationManager::new(migrated_pool.clone());

        let rsvp = make_a_reservation();
        let rsvp = manager.reserve(rsvp).await.unwrap();

        let rsvp = manager.change_status(rsvp.id.clone()).await.unwrap();
        assert_eq!(rsvp.status, ReservationStatus::Confirmed as i32);
    }

    #[sqlx_database_tester::test(pool(variable = "migrated_pool", migrations = "../migrations"))]
    async fn change_status_should_fail_with_invalid_id() {
        let manager = ReservationManager::new(migrated_pool.clone());

        let err = manager
            .change_status("invalid-id".to_string())
            .await
            .unwrap_err();
        assert_eq!(err, Error::InvalidReservationId("invalid-id".to_string()));
    }

    #[sqlx_database_tester::test(pool(variable = "migrated_pool", migrations = "../migrations"))]
    async fn update_note_should_work() {
        let manager = ReservationManager::new(migrated_pool.clone());

        let rsvp = make_a_reservation();
        let rsvp = manager.reserve(rsvp).await.unwrap();

        let rsvp = manager
            .update_note(rsvp.id.clone(), "new-note".to_string())
            .await
            .unwrap();
        assert_eq!(rsvp.note, "new-note".to_string());
    }

    #[sqlx_database_tester::test(pool(variable = "migrated_pool", migrations = "../migrations"))]
    async fn get_reservation_should_work() {
        let manager = ReservationManager::new(migrated_pool.clone());

        let rsvp = make_a_reservation();
        let rsvp = manager.reserve(rsvp).await.unwrap();

        let rsvp = manager.get(rsvp.id.clone()).await.unwrap();
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
        let manager = ReservationManager::new(migrated_pool.clone());

        let rsvp = make_a_reservation();
        let rsvp = manager.reserve(rsvp).await.unwrap();

        manager.delete(rsvp.id.clone()).await.unwrap();

        let err = manager.get(rsvp.id.clone()).await.unwrap_err();
        assert_eq!(err, Error::NotFound);
    }

    fn make_a_reservation() -> Reservation {
        Reservation::new(
            "test-user".to_string(),
            "test-resource".to_string(),
            "2023-1-1T10:10:10-0700".parse().unwrap(),
            "2023-1-4T10:10:10-0700".parse().unwrap(),
            "test-note".to_string(),
        )
    }
}

use std::{
    ops::Deref,
    pin::Pin,
    task::{Context, Poll},
};

use abi::{
    reservation_service_server::{ReservationService, ReservationServiceServer},
    CancelRequest, CancelResponse, Config, ConfirmRequest, ConfirmResponse, Error, FilterRequest,
    FilterResponse, GetRequest, GetResponse, ListenRequest, QueryRequest, ReserveRequest,
    ReserveResponse, UpdateRequest, UpdateResponse,
};
use futures::Stream;
use reservation::{ReservationManager, Rsvp};
use tokio::sync::mpsc;
use tonic::{async_trait, transport::Server, Request, Response, Status};

use crate::{ReservationStream, RsvpService, TonicReceiverStream};

pub async fn start_server(config: &Config) -> Result<(), anyhow::Error> {
    let addr = format!("{}:{}", config.server.host, config.server.port).parse()?;

    let service = RsvpService::from_config(config).await?;
    let service = ReservationServiceServer::new(service);

    println!("Listening on {}", addr);

    Server::builder().add_service(service).serve(addr).await?;

    Ok(())
}

impl Deref for RsvpService {
    type Target = ReservationManager;

    fn deref(&self) -> &Self::Target {
        &self.manager
    }
}

impl RsvpService {
    pub async fn from_config(config: &Config) -> Result<Self, Error> {
        Ok(Self {
            manager: ReservationManager::from_config(&config.db).await?,
        })
    }
}

#[async_trait]
impl ReservationService for RsvpService {
    async fn reserve(
        &self,
        request: Request<ReserveRequest>,
    ) -> Result<Response<ReserveResponse>, Status> {
        let request = request.into_inner();
        if request.reservation.is_none() {
            return Err(Error::MissingArgument("reservation".to_string()).into());
        }
        let reservation = self.manager.reserve(request.reservation.unwrap()).await?;
        Ok(Response::new(ReserveResponse {
            reservation: Some(reservation),
        }))
    }

    /// confirm a reservation
    async fn confirm(
        &self,
        request: Request<ConfirmRequest>,
    ) -> Result<Response<ConfirmResponse>, Status> {
        let request = request.into_inner();
        let rsvp = self.manager.change_status(request.id).await?;
        Ok(Response::new(ConfirmResponse {
            reservation: Some(rsvp),
        }))
    }

    /// update a reservation
    async fn update(
        &self,
        request: Request<UpdateRequest>,
    ) -> Result<Response<UpdateResponse>, Status> {
        let request = request.into_inner();
        let rsvp = self.manager.update_note(request.id, request.note).await?;
        Ok(Response::new(UpdateResponse {
            reservation: Some(rsvp),
        }))
    }

    ///  cancel a reservation
    async fn cancel(
        &self,
        request: Request<CancelRequest>,
    ) -> Result<Response<CancelResponse>, Status> {
        let request = request.into_inner();
        let rsvp = self.manager.delete(request.id).await?;
        Ok(Response::new(CancelResponse {
            reservation: Some(rsvp),
        }))
    }

    /// get a reservation
    async fn get(&self, request: Request<GetRequest>) -> Result<Response<GetResponse>, Status> {
        let request = request.into_inner();
        let rsvp = self.manager.get(request.id).await?;
        Ok(Response::new(GetResponse {
            reservation: Some(rsvp),
        }))
    }

    /// Server streaming response type for the query method.
    type queryStream = ReservationStream;

    /// query reservations
    async fn query(
        &self,
        request: Request<QueryRequest>,
    ) -> Result<Response<Self::queryStream>, Status> {
        let request = request.into_inner();
        if request.query.is_none() {
            return Err(Error::MissingArgument("missing argument: query".to_string()).into());
        }
        let rx = self.manager.query(request.query.unwrap()).await;
        let stream = TonicReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream)))
    }

    /// filter reservations
    async fn filter(
        &self,
        request: Request<FilterRequest>,
    ) -> Result<Response<FilterResponse>, Status> {
        let request = request.into_inner();
        if request.filter.is_none() {
            return Err(Error::MissingArgument("filter".to_string()).into());
        }
        let filter = request.filter.unwrap();
        let (pager, rsvps) = self.manager.filter(filter).await?;
        Ok(Response::new(FilterResponse {
            reservations: rsvps,
            pager: Some(pager),
        }))
    }

    /// Server streaming response type for the listen method.
    type listenStream = ReservationStream;

    /// listen to reservation changes
    async fn listen(
        &self,
        _request: Request<ListenRequest>,
    ) -> Result<Response<Self::listenStream>, Status> {
        todo!()
    }
}

impl<T> TonicReceiverStream<T> {
    pub fn new(inner: mpsc::Receiver<Result<T, Error>>) -> Self {
        Self { inner }
    }
}

/// 需要为 TonicReceiverStream 实现 futures::Stream trait
impl<T> Stream for TonicReceiverStream<T> {
    type Item = Result<T, Status>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.inner.poll_recv(cx) {
            Poll::Ready(Some(Ok(item))) => Poll::Ready(Some(Ok(item))),
            Poll::Ready(Some(Err(err))) => Poll::Ready(Some(Err(err.into()))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[cfg(test)]
mod tests {
    use abi::{Config, Reservation};
    use lazy_static::lazy_static;
    use sqlx::{postgres::PgConnection, Connection, Executor};
    use std::{sync::Arc, thread};
    use tokio::runtime::Runtime;
    use uuid::Uuid;

    use super::*;

    lazy_static! {
        /// Runtime is a tokio runtime for running async tests.
        /// runtime 使异步的接口可以在同步的测试中使用
        static ref TEST_RT: Runtime = Runtime::new().unwrap();
    }

    /// TestConfig is a helper struct for testing. It's for unbinding the database connection[sqlx]
    /// It creates a new database for each test and drops it after the test.
    struct TestConfig {
        config: Arc<Config>,
    }

    impl TestConfig {
        fn new() -> Self {
            let mut config = Config::load("fitures/config.yml").unwrap();
            let uuid = Uuid::new_v4();
            let old_url = config.db.url();
            let dbname = format!("test_{}", uuid);
            config.db.dbname = dbname.clone();

            let url = config.db.url();

            // create a thread to create database for test.
            thread::spawn(move || {
                TEST_RT.block_on(async move {
                    // connect for creating database
                    let mut conn = PgConnection::connect(&old_url).await.unwrap();
                    conn.execute(format!(r#"CREATE DATABASE "{}""#, dbname).as_str())
                        .await
                        .unwrap();
                    // connect for migration
                    let mut conn = PgConnection::connect(&url).await.unwrap();
                    sqlx::migrate!("../migrations")
                        .run(&mut conn)
                        .await
                        .unwrap();
                });
            })
            .join()
            .expect("failed to create database");

            Self {
                config: Arc::new(config),
            }
        }
    }

    /// Drop the database after the test.
    /// when finished the test, it drops the database.
    impl Drop for TestConfig {
        fn drop(&mut self) {
            let url = self.config.db.server_url();
            let dbname = self.config.db.dbname.clone();
            thread::spawn(move || {
                TEST_RT.block_on(async move {
                    let mut conn = PgConnection::connect(&url).await.unwrap();
                    // terminate all other connections。关闭数据库连接
                    sqlx::query(&format!(
                        r#"SELECT pg_terminate_backend(pid) FROM pg_stat_activity
                    WHERE pid <> pg_backend_pid() AND datname = '{}'"#,
                        dbname
                    ))
                    .execute(&mut conn)
                    .await
                    .expect("Terminate all other connections");
                    conn.execute(format!(r#"DROP DATABASE "{}""#, dbname).as_str())
                        .await
                        .expect("Error while querying the drop database");
                });
            })
            .join()
            .expect("failed to drop database");
        }
    }

    #[tokio::test]
    async fn rpc_reserve_should_work() {
        let config = TestConfig::new();
        let service = RsvpService::from_config(&config.config).await.unwrap();
        let reservation = Reservation::new(
            "xxl",
            "xxl-resource",
            "2022-02-01T15:00:01-0800".parse().unwrap(),
            "2022-02-04T15:00:01-0800".parse().unwrap(),
            "xxl-note",
        );
        let request = tonic::Request::new(ReserveRequest {
            reservation: Some(reservation.clone()),
        });
        let resp = service.reserve(request).await.unwrap();
        let rsvp = resp.into_inner().reservation;
        assert!(rsvp.is_some());
        let rsvp = rsvp.unwrap();
        assert_eq!(rsvp.user_id, reservation.user_id);
        assert_eq!(rsvp.resource_id, reservation.resource_id);
        assert_eq!(rsvp.note, reservation.note);
        assert_eq!(rsvp.start, reservation.start);
        assert_eq!(rsvp.status, reservation.status);
        assert_eq!(rsvp.end, reservation.end);
    }
}

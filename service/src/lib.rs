use abi::{
    reservation_service_server::{ReservationService, ReservationServiceServer},
    CancelRequest, CancelResponse, Config, ConfirmRequest, ConfirmResponse, Error, FilterRequest,
    FilterResponse, GetRequest, GetResponse, ListenRequest, QueryRequest, Reservation,
    ReserveRequest, ReserveResponse, UpdateRequest, UpdateResponse,
};
use futures::Stream;
use reservation::{ReservationManager, Rsvp};
use std::{ops::Deref, pin::Pin};
use tonic::{async_trait, transport::Server, Request, Response, Status};

pub struct RsvpService {
    manager: ReservationManager,
}

type ReservationStream = Pin<Box<dyn Stream<Item = Result<Reservation, Status>> + Send>>;

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
        if request.reservation.is_some() {
            return Err(Status::invalid_argument("missing reservation"));
        }
        let reservation = self.manager.reserve(request.reservation.unwrap()).await?;
        Ok(Response::new(ReserveResponse {
            reservation: Some(reservation),
        }))
    }

    /// confirm a reservation
    async fn confirm(
        &self,
        _request: Request<ConfirmRequest>,
    ) -> Result<Response<ConfirmResponse>, Status> {
        todo!()
    }
    /// update a reservation
    async fn update(
        &self,
        _request: Request<UpdateRequest>,
    ) -> Result<Response<UpdateResponse>, Status> {
        todo!()
    }
    ///  cancel a reservation
    async fn cancel(
        &self,
        _request: Request<CancelRequest>,
    ) -> Result<Response<CancelResponse>, Status> {
        todo!()
    }
    /// get a reservation
    async fn get(&self, _request: Request<GetRequest>) -> Result<Response<GetResponse>, Status> {
        todo!()
    }
    /// Server streaming response type for the query method.
    type queryStream = ReservationStream;
    /// query reservations
    async fn query(
        &self,
        _request: Request<QueryRequest>,
    ) -> Result<Response<Self::queryStream>, Status> {
        todo!()
    }
    /// filter reservations
    async fn filter(
        &self,
        _request: Request<FilterRequest>,
    ) -> Result<Response<FilterResponse>, Status> {
        todo!()
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

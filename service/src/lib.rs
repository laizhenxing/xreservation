mod service;

use abi::{Error, Reservation};
use futures::Stream;
use reservation::ReservationManager;
use std::pin::Pin;
use tokio::sync::mpsc;
use tonic::Status;

pub use service::*;

pub struct RsvpService {
    manager: ReservationManager,
}

/// use struct to wrap the receiver stream
/// see the doc of `tokio_stream::wrappers::ReceiverStream`
pub struct TonicReceiverStream<T> {
    inner: mpsc::Receiver<Result<T, Error>>,
}

type ReservationStream = Pin<Box<dyn Stream<Item = Result<Reservation, Status>> + Send>>;

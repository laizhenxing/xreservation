/// 引入 test_utils 模块
#[path = "../src/test_utils.rs"]
mod test_utils;

use std::time::Duration;

use abi::{
    reservation_service_client::ReservationServiceClient, CancelRequest, Config, ConfirmRequest,
    FilterRequest, GetRequest, QueryRequest, Reservation, ReservationFilterBuilder,
    ReservationQueryBuilder, ReserveRequest, UpdateRequest,
};
use futures::StreamExt;
use reservation_service::start_server;
use tokio::time;

use test_utils::TestConfig;

#[tokio::test]
async fn grpc_reserve_should_work() {
    let config = TestConfig::with_server_port(50001);
    let mut client = get_test_cliet(&config).await;

    // 1. first reserve a reservation
    let mut rsvp1 = Reservation::new(
        "test-uid",
        "test-rid",
        "2023-01-09T10:10:10-0800".parse().unwrap(),
        "2023-01-10T10:10:10-0800".parse().unwrap(),
        "test-node",
    );
    let ret1 = client
        .reserve(ReserveRequest::new(rsvp1.clone()))
        .await
        .unwrap()
        .into_inner()
        .reservation
        .unwrap();
    rsvp1.id = ret1.id;
    assert_eq!(ret1, rsvp1);

    // 2. then reserve a conflict reservation. should fail
    let rsvp2 = Reservation::new(
        "test-uid2",
        "test-rid",
        "2023-01-09T10:10:10-0800".parse().unwrap(),
        "2023-01-10T10:10:10-0800".parse().unwrap(),
        "test-node2",
    );
    let ret2 = client.reserve(ReserveRequest::new(rsvp2.clone())).await;
    assert!(ret2.is_err());
    assert_eq!(ret2.unwrap_err().code(), tonic::Code::AlreadyExists);

    // 3. confirm the first reservation
    let ret3 = client
        .confirm(ConfirmRequest::new(rsvp1.id))
        .await
        .unwrap()
        .into_inner()
        .reservation
        .unwrap();
    assert_eq!(ret3.status, abi::ReservationStatus::Confirmed as i32);
}

#[tokio::test]
async fn grpc_query_should_work() {
    let config = TestConfig::with_server_port(50002);
    let mut client = get_test_cliet(&config).await;
    make_reservations(&mut client, 10).await;

    let query = ReservationQueryBuilder::default()
        .resource_id("test-rid-1")
        .start("2023-01-09T10:10:10-0800".parse().unwrap())
        .end("2023-01-10T10:10:10-0800".parse().unwrap())
        .build()
        .unwrap();
    let mut ret = client
        .query(QueryRequest::new(query))
        .await
        .unwrap()
        .into_inner();
    while let Some(Ok(rsvp)) = ret.next().await {
        assert_eq!(rsvp.user_id, "test-uid-1");
        assert_eq!(rsvp.resource_id, "test-rid-1");
        assert_eq!(rsvp.note, "test-node-1");
    }
}

#[tokio::test]
async fn grpc_cancel_should_work() {
    let config = TestConfig::with_server_port(50003);
    let mut client = get_test_cliet(&config).await;
    make_reservations(&mut client, 10).await;

    let ret = client
        .cancel(CancelRequest::new(1))
        .await
        .unwrap()
        .into_inner()
        .reservation
        .unwrap();
    assert_eq!(ret.user_id, "yuzhe");
    assert_eq!(ret.resource_id, "test-rid-1");
    assert_eq!(ret.status, abi::ReservationStatus::Pending as i32);
}

#[tokio::test]
async fn grpc_get_should_work() {
    let config = TestConfig::with_server_port(50004);
    let mut client = get_test_cliet(&config).await;
    make_reservations(&mut client, 10).await;

    let ret = client
        .get(GetRequest::new(6))
        .await
        .unwrap()
        .into_inner()
        .reservation
        .unwrap();

    assert_eq!(ret.user_id, "yuzhe");
    assert_eq!(ret.resource_id, "test-rid-6");
    assert_eq!(ret.status, abi::ReservationStatus::Pending as i32);
    assert_eq!(ret.note, "test-node-6");
}

#[tokio::test]
async fn grpc_update_should_work() {
    let config = TestConfig::with_server_port(50005);
    let mut client = get_test_cliet(&config).await;
    make_reservations(&mut client, 10).await;

    let ret = client
        .update(UpdateRequest::new(6, "test-node-6-updated".to_string()))
        .await
        .unwrap()
        .into_inner()
        .reservation
        .unwrap();

    assert_eq!(ret.user_id, "yuzhe");
    assert_eq!(ret.resource_id, "test-rid-6");
    assert_eq!(ret.status, abi::ReservationStatus::Pending as i32);
    assert_eq!(ret.note, "test-node-6-updated");
}

#[tokio::test]
async fn grpc_filter_should_work() {
    let config = TestConfig::with_server_port(50006);
    let mut client = get_test_cliet(&config).await;
    make_reservations(&mut client, 100).await;

    let filter = ReservationFilterBuilder::default()
        .user_id("yuzhe")
        .status(abi::ReservationStatus::Pending as i32)
        .cursor(4)
        .page_size(14)
        .build()
        .unwrap();
    let ret = client
        .filter(FilterRequest::new(filter))
        .await
        .unwrap()
        .into_inner();
    let rsvps = ret.reservations;
    let pager = ret.pager.unwrap();

    assert_eq!(rsvps.len(), 14);
    assert_eq!(rsvps[0].id, 5);
    assert_eq!(rsvps[13].id, 18);

    assert_eq!(pager.prev, -1);
    assert_eq!(pager.next, 18);
}

async fn get_test_cliet(
    config: &TestConfig,
) -> ReservationServiceClient<tonic::transport::Channel> {
    let config = &config.config;
    setup_server(config);

    let fut = async move {
        while ReservationServiceClient::connect(config.server.url(false))
            .await
            .is_err()
        {
            time::sleep(Duration::from_millis(10)).await;
        }
        ReservationServiceClient::connect(config.server.url(false))
            .await
            .unwrap()
    };

    time::timeout(Duration::from_secs(5), fut).await.unwrap()
}

fn setup_server(config: &Config) {
    let config_cloned = config.clone();
    tokio::spawn(async move {
        start_server(&config_cloned).await.unwrap();
    });
}

async fn make_reservations(
    client: &mut ReservationServiceClient<tonic::transport::Channel>,
    count: u32,
) {
    for i in 0..count {
        let mut rsvp = Reservation::new(
            "yuzhe",
            &format!("test-rid-{}", i + 1),
            "2023-01-09T10:10:10-0800".parse().unwrap(),
            "2023-01-10T10:10:10-0800".parse().unwrap(),
            &format!("test-node-{}", i + 1),
        );
        let ret = client
            .reserve(ReserveRequest::new(rsvp.clone()))
            .await
            .unwrap()
            .into_inner()
            .reservation
            .unwrap();
        rsvp.id = ret.id;
        assert_eq!(ret, rsvp);
    }
}

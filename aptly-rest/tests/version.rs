use aptly_rest::AptlyRest;
use aptly_rest_mock::AptlyRestMock;

#[tokio::test]
async fn test_version() {
    let mock = AptlyRestMock::start().await;
    let aptly = AptlyRest::new(mock.url());
    let version = aptly.version().await.unwrap();
    assert_eq!(version, aptly_rest_mock::APTLY_VERSION);
}

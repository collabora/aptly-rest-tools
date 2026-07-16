use aptly_rest::AptlyRest;
use aptly_rest_mock::AptlyRestMock;

#[tokio::test]
async fn mirrors() {
    let mock = AptlyRestMock::start().await;
    mock.load_default_data();

    let aptly = AptlyRest::new(mock.url());

    let expected = mock.mirrors();
    let received = aptly.mirrors().await.expect("failed to get mirrors");

    assert_eq!(expected.len(), received.len());
    for e in &expected {
        let m = received.iter().find(|m| m.name() == e.name()).unwrap();
        assert_eq!(m.uuid(), e.uuid());
        assert_eq!(m.distribution(), e.distribution());
        assert_eq!(m.components(), e.components());
        assert_eq!(m.architectures(), e.architectures());
    }
}

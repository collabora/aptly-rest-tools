use aptly_rest::AptlyRest;
use aptly_rest_mock::AptlyRestMock;
use url::Url;

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

#[tokio::test]
async fn mirror_create() {
    let mock = AptlyRestMock::start().await;
    let aptly = AptlyRest::new(mock.url());

    let url: Url = "https://example.com/debian/".parse().unwrap();
    let mirror = aptly.mirror("test-mirror");
    let mut creation = mirror.create(url);
    creation
        .distribution("bookworm")
        .filter("Name (% nginx*)")
        .components(vec!["main".to_owned(), "contrib".to_owned()])
        .architectures(vec!["amd64".to_owned(), "arm64".to_owned()])
        .keyrings(vec!["/etc/apt/trusted.gpg".to_owned()])
        .ignore_signatures(true)
        .download_sources(true)
        .download_udebs(true)
        .download_installer(true)
        .download_app_stream(true)
        .filter_with_deps(true)
        .skip_component_check(true)
        .skip_architecture_check(true);

    let created = creation.run().await.expect("failed to create mirror");

    // The response echoes back the created mirror.
    assert_eq!(created.name(), "test-mirror");
    assert_eq!(created.distribution(), "bookworm");
    assert_eq!(created.components(), ["main", "contrib"]);
    assert_eq!(created.architectures(), ["amd64", "arm64"]);
    assert!(created.skip_architecture_check());

    // The mock recorded every option that was sent.
    let mirrors = mock.mirrors();
    let m = mirrors.get("test-mirror").expect("mirror not stored");
    assert_eq!(m.distribution(), "bookworm");
    assert_eq!(m.filter(), "Name (% nginx*)");
    assert_eq!(m.components(), ["main", "contrib"]);
    assert_eq!(m.architectures(), ["amd64", "arm64"]);
    assert_eq!(m.keyrings(), ["/etc/apt/trusted.gpg"]);
    assert!(m.ignore_signatures());
    assert!(m.download_sources());
    assert!(m.download_udebs());
    assert!(m.download_installer());
    assert!(m.download_app_stream());
    assert!(m.filter_with_deps());
    assert!(m.skip_component_check());
    assert!(m.skip_architecture_check());
}

#[tokio::test]
async fn mirror_update() {
    let mock = AptlyRestMock::start().await;
    mock.load_default_data();
    let aptly = AptlyRest::new(mock.url());

    let mirror = aptly.mirror("apertis-v2023pre");
    let mut update = mirror.update();
    update
        .rename("apertis-renamed")
        .keyrings(vec!["/etc/apt/trusted.gpg".to_owned()])
        .ignore_checksums(true)
        .ignore_signatures(true)
        .force_update(true)
        .skip_existing_packages(true)
        .latest_only(true);
    update.run().await.expect("failed to update mirror");

    let mirrors = mock.mirrors();
    let m = mirrors.get("apertis-v2023pre").expect("mirror missing");
    let u = m.applied_update().expect("no update recorded");
    assert_eq!(u.rename.as_deref(), Some("apertis-renamed"));
    assert_eq!(u.keyrings, ["/etc/apt/trusted.gpg"]);
    assert!(u.ignore_checksums);
    assert!(u.ignore_signatures);
    assert!(u.force_update);
    assert!(u.skip_existing_packages);
    assert!(u.latest_only);
}

#[tokio::test]
async fn mirror_update_rejects_unknown_fields() {
    let mock = AptlyRestMock::start().await;
    mock.load_default_data();

    // A field the update (PUT) endpoint does not accept -- e.g. a config-edit field --
    // must be rejected rather than silently ignored.
    let url = mock.url().join("api/mirrors/apertis-v2023pre").unwrap();
    let response = reqwest::Client::new()
        .put(url)
        .json(&serde_json::json!({ "ForceUpdate": true, "ArchiveUrl": "http://example.com/" }))
        .send()
        .await
        .expect("request failed");

    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
}

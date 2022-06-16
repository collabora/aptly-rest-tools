use std::str::FromStr;

use aptly_rest::{key::AptlyKey, AptlyRest};
use aptly_rest_mock::AptlyRestMock;

fn none_if_empty(v: &str) -> Option<&str> {
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}

#[tokio::test]
async fn repos() {
    let mock = AptlyRestMock::start().await;
    mock.load_default_data();

    let aptly = AptlyRest::new(mock.url());

    let expected = mock.repos();
    let received = aptly.repos().await.expect("failed to get repositories");

    assert_eq!(expected.len(), received.len());
    for e in &expected {
        let r = received.iter().find(|r| r.name() == e.name).unwrap();
        assert_eq!(r.comment(), none_if_empty(&e.comment));
        assert_eq!(r.distribution(), none_if_empty(&e.distribution));
        assert_eq!(r.component(), none_if_empty(&e.component));
    }
}

#[tokio::test]
async fn repo_packages_list() {
    let mock = AptlyRestMock::start().await;
    mock.load_default_data();

    let repos = mock.repos();
    let repo = repos.get("bullseye-repo").unwrap();
    let repo_packages = repo.packages();

    let aptly = AptlyRest::new(mock.url());
    let packages = aptly.repo("bullseye-repo").packages().list().await.unwrap();

    assert_eq!(repo_packages.len(), packages.len());
    for p in packages {
        assert!(repo_packages
            .iter()
            .find(|r| p == AptlyKey::from_str(r).unwrap())
            .is_some())
    }
}

#[tokio::test]
async fn repo_packages_detailed() {
    let mock = AptlyRestMock::start().await;
    mock.load_default_data();

    let repos = mock.repos();
    let repo = repos.get("bullseye-repo").unwrap();
    let repo_packages = repo.packages();

    let aptly = AptlyRest::new(mock.url());
    let packages = aptly
        .repo("bullseye-repo")
        .packages()
        .detailed()
        .await
        .unwrap();

    assert_eq!(repo_packages.len(), packages.len());
    for p in packages {
        let key_s = p.key().to_string();
        assert!(repo_packages.contains(&key_s));
    }
}

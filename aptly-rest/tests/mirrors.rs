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
async fn mirrors() {
    let mock = AptlyRestMock::start().await;
    mock.load_default_data();

    let aptly = AptlyRest::new(mock.url());

    let expected = mock.mirrors();
    let received = aptly.mirrors().await.expect("failed to get mirrors");

    assert_eq!(expected.len(), received.len());
    for e in &expected {
        let m = received.iter().find(|m| m.name == e.name()).unwrap();
        assert_eq!(m.uuid, e.uuid());
        assert_eq!(m.distribution, e.distribution());
        assert_eq!(m.components, e.components());
        assert_eq!(m.architectures, e.architectures());
    }
}

/*
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
            .any(|r| p == AptlyKey::from_str(r).unwrap()))
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
*/

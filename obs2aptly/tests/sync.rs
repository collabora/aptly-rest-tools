use std::{
    fs::File,
    path::{Path, PathBuf},
};

use anyhow::{bail, Result};
use aptly_rest::AptlyRest;
use aptly_rest_mock::AptlyRestMock;
use obs2aptly::{AptlyContent, ObsContent, SyncAction};
use serde::{Deserialize, Serialize};

fn data_path<P0: AsRef<Path>, P1: AsRef<Path>>(subdir: P0, file: P1) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/data");
    path.push(subdir);
    path.push(file);
    path
}

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Hash)]
enum ExpectedAction {
    Must(SyncAction),
    OneOf(Vec<SyncAction>),
}

impl ExpectedAction {
    fn match_action(expected: &SyncAction, action: &SyncAction, path_prefix: &Path) -> bool {
        match (expected, action) {
            (SyncAction::AddDeb(e), SyncAction::AddDeb(a)) => {
                a.strip_prefix(path_prefix).unwrap() == e
            }
            (SyncAction::AddDsc(e), SyncAction::AddDsc(a)) => {
                a.strip_prefix(path_prefix).unwrap() == e
            }
            (SyncAction::RemoveAptly(e), SyncAction::RemoveAptly(a)) => e == a,
            _ => false,
        }
    }

    fn matches_action(&self, action: &SyncAction, path_prefix: &Path) -> bool {
        match self {
            ExpectedAction::Must(e) => Self::match_action(e, action, path_prefix),
            ExpectedAction::OneOf(options) => {
                for e in options {
                    if Self::match_action(e, action, path_prefix) {
                        return true;
                    }
                }
                false
            }
        }
    }
}

fn load_expected_actions(path: &Path) -> Vec<ExpectedAction> {
    let f = File::open(path).expect("Couldn't load expected actions");
    serde_json::from_reader(f).expect("Couldn't parse expected actions")
}

fn compare_actions(
    actual: &[SyncAction],
    mut expected: Vec<ExpectedAction>,
    path_prefix: &Path,
) -> Result<()> {
    let mut r = Ok(());

    for action in actual {
        let found = expected
            .iter()
            .enumerate()
            .find(|(_, e)| e.matches_action(&action, path_prefix));
        if let Some((i, _)) = found {
            expected.swap_remove(i);
        } else {
            eprintln!("- Unexpected action: {:?}", action);
            r = Err(anyhow::anyhow!("Actions didn't match"));
        }
    }

    for action in expected {
        eprintln!("- Missing action: {:?}", action);
        r = Err(anyhow::anyhow!("Actions didn't match"));
    }

    r
}

async fn run_test<P: AsRef<Path>>(path: P, repo: &str) {
    let mock = AptlyRestMock::start().await;
    mock.load_data(&data_path(&path, "aptly.json"));

    let aptly = AptlyRest::new(mock.url());

    let aptly_contents = AptlyContent::new_from_aptly(&aptly, repo).await.unwrap();

    let obs_path = data_path(&path, "obs");
    let obs_content = ObsContent::new_from_path(obs_path.clone()).await.unwrap();

    let actions = obs2aptly::sync(aptly, obs_content, aptly_contents)
        .await
        .unwrap();
    let expected = load_expected_actions(&data_path(&path, "expected.json"));
    compare_actions(actions.actions(), expected, &obs_path).unwrap();
}

#[tokio::test]
async fn empty_aptly() {
    run_test("empty_aptly", "empty").await;
}

#[tokio::test]
async fn empty_obs() {
    run_test("empty_obs", "bullseye").await;
}

#[tokio::test]
async fn simple_updates() {
    run_test("simple_updates", "bullseye").await;
}

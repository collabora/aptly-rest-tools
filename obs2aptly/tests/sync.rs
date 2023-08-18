use std::{
    ffi::OsStr,
    fs::{self, File},
    io::BufReader,
    path::{Path, PathBuf},
};

use aptly_rest::{key::AptlyKey, AptlyRest};
use aptly_rest_mock::AptlyRestMock;
use color_eyre::{eyre::eyre, Result};
use debian_packaging::{control::ControlFile, deb::builder::DebBuilder};
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use sync2aptly::{AptlyContent, SyncAction};

fn data_path<P0: AsRef<Path>, P1: AsRef<Path>>(subdir: P0, file: P1) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/data");
    path.push(subdir);
    path.push(file);
    path
}

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Hash)]
enum ExpectedSyncAction {
    AddDeb(PathBuf),
    AddDsc(Vec<PathBuf>),
    RemoveAptly(AptlyKey),
}

#[derive(Debug, Deserialize, Serialize, Eq, PartialEq, Hash)]
enum ExpectedAction {
    Must(ExpectedSyncAction),
    OneOf(Vec<ExpectedSyncAction>),
}

impl ExpectedAction {
    fn match_action(
        expected: &ExpectedSyncAction,
        action: &SyncAction,
        path_prefix: &Path,
    ) -> bool {
        match (expected, action) {
            (ExpectedSyncAction::AddDeb(e), SyncAction::AddDeb { location, .. }) => {
                location
                    .as_path()
                    .unwrap()
                    .strip_prefix(path_prefix)
                    .unwrap()
                    == e
            }
            (ExpectedSyncAction::AddDsc(e), SyncAction::AddDsc { dsc_location, .. }) => {
                dsc_location
                    .as_path()
                    .unwrap()
                    .strip_prefix(path_prefix)
                    .unwrap()
                    == e[0]
            }
            (ExpectedSyncAction::RemoveAptly(e), SyncAction::RemoveAptly(a)) => e == a,
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
            .find(|(_, e)| e.matches_action(action, path_prefix));
        if let Some((i, _)) = found {
            expected.swap_remove(i);
        } else {
            eprintln!("- Unexpected action: {:?}", action);
            r = Err(eyre!("Actions didn't match"));
        }
    }

    for action in expected {
        eprintln!("- Missing action: {:?}", action);
        r = Err(eyre!("Actions didn't match"));
    }

    r
}

static TRACING_INIT: OnceCell<()> = OnceCell::new();

async fn run_test<P: AsRef<Path>>(path: P, repo: &str) {
    TRACING_INIT.get_or_init(|| {
        tracing_subscriber::fmt::init();
    });
    let mock = AptlyRestMock::start().await;
    mock.load_data(&data_path(&path, "aptly.json"));

    let aptly = AptlyRest::new(mock.url());

    let aptly_contents = AptlyContent::new_from_aptly(&aptly, repo.to_owned())
        .await
        .unwrap();

    let obs_path = data_path(&path, "obs");
    let obs_temp_dir = tempfile::tempdir().unwrap();

    for entry in std::fs::read_dir(&obs_path).unwrap() {
        let entry = entry.unwrap();
        let dest = obs_temp_dir.path().join(entry.file_name());
        if dest.extension() == Some(OsStr::new("control")) {
            let control_file = File::open(entry.path()).unwrap();
            let mut control_rd = BufReader::new(control_file);
            let control = ControlFile::parse_reader(&mut control_rd).unwrap();

            let package_name = control
                .paragraphs()
                .next()
                .unwrap()
                .required_field_str("Package")
                .unwrap();
            let is_udeb = package_name.ends_with("-udeb");

            let deb = DebBuilder::new(control);

            let dest = dest.with_extension(if is_udeb { "udeb" } else { "deb" });
            let mut dest_file = File::create(dest).unwrap();
            deb.write(&mut dest_file).unwrap();
        } else {
            fs::copy(entry.path(), obs_temp_dir.path().join(entry.file_name())).unwrap();
        }
    }

    let actions = obs2aptly::sync(obs_temp_dir.path().to_owned(), aptly, aptly_contents)
        .await
        .unwrap();
    let expected = load_expected_actions(&data_path(&path, "expected.json"));
    compare_actions(actions.actions(), expected, obs_temp_dir.path()).unwrap();
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

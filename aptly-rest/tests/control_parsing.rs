use aptly_rest::dsc::Dsc;
use aptly_rest::key::AptlyKey;
use aptly_rest::utils::scanner::{self, Scanner};
use futures::TryStreamExt;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, BufReader};

fn data_path<P: AsRef<Path>>(file: P) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/data");
    path.push(file);
    path
}

async fn read_keys(path: &Path) -> HashSet<AptlyKey> {
    let path = match path.extension() {
        Some(p) => path.with_extension(format!("{}.keys", p.to_str().unwrap())),
        None => path.with_extension("keys"),
    };
    println!("Opening {}", path.display());

    let f = File::open(path).await.unwrap();
    let mut f = BufReader::new(f);
    let mut set = HashSet::new();
    loop {
        let mut s = String::new();
        if f.read_line(&mut s).await.unwrap() == 0 {
            break;
        }
        println!("Parsing: \"{}\"", s.trim_end());
        let key = s.trim_end().parse().unwrap();
        set.insert(key);
    }

    set
}

async fn test_dsc<P: AsRef<Path>>(file: P) {
    let path = data_path(file);
    let mut keys = read_keys(&path).await;
    assert_eq!(keys.len(), 1);
    let expected = keys.drain().next().unwrap();

    let dsc = Dsc::from_file(path).await.unwrap();
    let key = AptlyKey::try_from(&dsc).unwrap();
    assert_eq!(key, expected, "Got: {} Expected: {}", key, expected);
}

#[tokio::test]
async fn signed_dsc() {
    test_dsc("systemd_247.3-7.dsc").await;
}

#[tokio::test]
async fn dsc() {
    test_dsc("systemd_247.3-6+apertis4.dsc").await;
}

#[tokio::test]
async fn changes() {
    // TODO fix
    /*
    let path = data_path("systemd_247.3-6+apertis4bv2023dev2b6_arm64.changes");
    let changes = Changes::from_file(path.clone()).await.unwrap();
    let mut keys = read_keys(&path).await;
    for file in changes.files().unwrap() {
        let key = match AptlyKey::try_from(&file) {
            Ok(key) => key,
            Err(ChangesFileToAptlyKeyError::UnsupportPackageType) => continue,
            Err(e) => panic!("{:?}", e),
        };
        if !keys.remove(&key) {
            let package = key.package();
            if let Some(expected) = keys.iter().find(|k| k.package() == package) {
                assert_eq!(&key, expected);
            } else {
                panic!("Package not found in keys: {}", key);
            }
        }
    }

    if !keys.is_empty() {
        panic!("Left-over files: {:?}", keys);
    }
    */
}

#[tokio::test]
async fn scanner() {
    let path = data_path("");
    let mut scanner = Scanner::new(path);
    let mut found: Vec<String> = Vec::new();

    while let Some(control) = scanner.try_next().await.unwrap() {
        let path = match &control {
            scanner::Found::Changes(c) => c.path(),
            scanner::Found::Dsc(d) => d.path(),
        };
        found.push(path.file_name().unwrap().to_string_lossy().into_owned())
    }

    assert_eq!(found.len(), 3);
    for item in [
        "systemd_247.3-7.dsc",
        "systemd_247.3-6+apertis4.dsc",
        "systemd_247.3-6+apertis4bv2023dev2b6_arm64.changes",
    ] {
        assert!(
            found.iter().any(|s| s.as_str() == item),
            "{} not in {:#?}",
            item,
            found
        );
    }
}

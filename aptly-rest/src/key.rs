use std::{fmt::Display, str::FromStr};

use debian_packaging::package_version::PackageVersion;
use serde_with::{DeserializeFromStr, SerializeDisplay};

#[derive(
    Debug, Clone, PartialEq, Eq, Ord, PartialOrd, Hash, DeserializeFromStr, SerializeDisplay,
)]
pub struct AptlyKey {
    package: String,
    version: PackageVersion,
    arch: String,
    hash: String,
}

impl Display for AptlyKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "P{} {} {} {}",
            self.arch, self.package, self.version, self.hash
        )
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ParseError {
    #[error("Invalid aptly key")]
    InvalidKey,
    #[error("Invalid architecture field")]
    InvalidArchitecture,
    #[error("Invalid package field")]
    InvalidPackage,
    #[error("Invalid version field: {0}")]
    InvalidVersion(#[from] debian_packaging::error::DebianError),
    #[error("Invalid hash field")]
    InvalidHash,
}

impl FromStr for AptlyKey {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s.starts_with('P') {
            return Err(ParseError::InvalidKey);
        }
        let mut parts = s.split(' ');

        let arch = parts.next().ok_or(ParseError::InvalidKey)?;
        /* Skip P prefix */
        let arch = &arch[1..];
        if arch.len() < 3 {
            return Err(ParseError::InvalidArchitecture);
        }

        let package = parts.next().ok_or(ParseError::InvalidKey)?;
        if package.is_empty() {
            return Err(ParseError::InvalidPackage);
        }

        let version = parts.next().ok_or(ParseError::InvalidKey)?;
        let version = PackageVersion::parse(version)?;

        let hash = parts.next().ok_or(ParseError::InvalidKey)?;
        /* Aptly doesn't show print 0 prefixes.. */
        if hash.len() > 16 {
            return Err(ParseError::InvalidHash);
        }
        /* TODO is aplty happy with uppercase hash digits */
        if hash.contains(|c: char| !c.is_ascii_hexdigit()) {
            return Err(ParseError::InvalidHash);
        }

        if parts.next().is_some() {
            return Err(ParseError::InvalidKey);
        }

        Ok(Self::new(
            arch.to_string(),
            package.to_string(),
            version,
            hash.to_lowercase(),
        ))
    }
}

impl AptlyKey {
    pub fn new(arch: String, package: String, version: PackageVersion, hash: String) -> Self {
        Self {
            arch,
            package,
            version,
            hash,
        }
    }

    /// Get a reference to the aptly key's architecture.
    pub fn arch(&self) -> &str {
        &self.arch
    }

    /// Get a reference to the aptly key's package.
    pub fn package(&self) -> &str {
        self.package.as_ref()
    }

    /// Get a reference to the aptly key's version.
    pub fn version(&self) -> &PackageVersion {
        &self.version
    }

    /// true if the package is a source package {
    pub fn is_source(&self) -> bool {
        self.arch == "source"
    }

    /// true if the package is a binary package {
    pub fn is_binary(&self) -> bool {
        self.arch != "source"
    }

    pub fn hash(&self) -> &str {
        self.hash.as_ref()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use debian_packaging::package_version::PackageVersion;

    #[test]
    fn display() {
        let key = AptlyKey::new(
            "amd64".to_string(),
            "aptly".to_string(),
            PackageVersion::parse("1.4.0+ds1-4").unwrap(),
            "decafbaddecafbad".to_string(),
        );

        assert_eq!("Pamd64 aptly 1.4.0+ds1-4 decafbaddecafbad", key.to_string());
    }

    #[test]
    fn parse() {
        let key = "Pamd64 aptly 1.4.0+ds1-4 decafbaddecafbad";
        let parsed: AptlyKey = key.parse().unwrap();
        let expected = AptlyKey::new(
            "amd64".to_string(),
            "aptly".to_string(),
            PackageVersion::parse("1.4.0+ds1-4").unwrap(),
            "decafbaddecafbad".to_string(),
        );

        assert_eq!(parsed, expected);
        assert_eq!(parsed.to_string(), key);
    }

    macro_rules! parse_fail {
        ($name: ident, $s: expr, $expected: pat) => {
            paste::paste! {
                #[test]
                fn [<parse_fail_ $name>]() {
                    let p: Result<AptlyKey, _> = $s.parse();
                    let e = p.unwrap_err();
                    if !matches!(e, $expected) {
                        panic!("\"{}\" expected error {} got {:?}", $s, stringify!($expected), e);
                    }
                }
            }
        };
    }
    parse_fail!(empty, "", ParseError::InvalidKey);
    parse_fail!(
        empty_architecture,
        "P a 5 12345678abcdabcd",
        ParseError::InvalidArchitecture
    );
    parse_fail!(
        invalid_version,
        "Pamd64 a ::version 12345678abcdabcd",
        ParseError::InvalidVersion(_)
    );
    parse_fail!(
        non_hex_hash,
        "Parch package version invalidhash12345",
        ParseError::InvalidHash
    );
    parse_fail!(
        missing_fields,
        "Parch package version",
        ParseError::InvalidKey
    );
    parse_fail!(
        too_many_fields,
        "Parch package version decafbaddecafbad whut",
        ParseError::InvalidKey
    );

    #[test]
    fn ord() {
        let key_a: AptlyKey = "Pamd64 alpha 5 beef".parse().unwrap();
        let key_b_0: AptlyKey = "Pamd64 beta 0 beef".parse().unwrap();
        let key_b_1: AptlyKey = "Pamd64 beta 1 beef".parse().unwrap();
        let key_b_1_mipsel: AptlyKey = "Pmipsel beta 1 beef".parse().unwrap();
        let key_b_1_mipsel_ffff: AptlyKey = "Pmipsel beta 1 fff".parse().unwrap();

        assert!(key_a < key_b_0);
        assert!(key_b_0 < key_b_1);
        assert!(key_b_1 < key_b_1_mipsel);
        assert!(key_b_1_mipsel < key_b_1_mipsel_ffff);
    }
}

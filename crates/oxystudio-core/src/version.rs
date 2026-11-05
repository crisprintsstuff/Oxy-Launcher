use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;

use crate::error::{Error, Result};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub pre: Option<String>,
}

impl Version {
    pub fn new(major: u64, minor: u64, patch: u64, pre: Option<String>) -> Self {
        Self {
            major,
            minor,
            patch,
            pre,
        }
    }

    /// Parse a version string, which may have a leading 'v'.
    /// Examples: "v1.2.3", "0.4.0-alpha", "1.0.0"
    pub fn parse(s: &str) -> Result<Self> {
        let trimmed = s.trim().strip_prefix('v').unwrap_or(s.trim());
        let (num_part, pre_part) = match trimmed.split_once('-') {
            Some((num, pre)) => (num, Some(pre.to_string())),
            None => (trimmed, None),
        };

        let parts: Vec<&str> = num_part.split('.').collect();
        if parts.is_empty() || parts.len() > 3 {
            return Err(Error::VersionParse(format!("invalid version string: {s}")));
        }

        let major = parts[0]
            .parse::<u64>()
            .map_err(|_| Error::VersionParse(format!("invalid major version in: {s}")))?;

        let minor = if parts.len() > 1 {
            parts[1]
                .parse::<u64>()
                .map_err(|_| Error::VersionParse(format!("invalid minor version in: {s}")))?
        } else {
            0
        };

        let patch = if parts.len() > 2 {
            parts[2]
                .parse::<u64>()
                .map_err(|_| Error::VersionParse(format!("invalid patch version in: {s}")))?
        } else {
            0
        };

        Ok(Self {
            major,
            minor,
            patch,
            pre: pre_part,
        })
    }

    pub fn is_prerelease(&self) -> bool {
        self.pre.is_some()
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if let Some(pre) = &self.pre {
            write!(f, "-{}", pre)?;
        }
        Ok(())
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch)) {
            Ordering::Equal => {
                match (&self.pre, &other.pre) {
                    // Both have no pre-release: equal
                    (None, None) => Ordering::Equal,
                    // Pre-release has lower precedence than normal release with same version
                    (Some(_), None) => Ordering::Less,
                    (None, Some(_)) => Ordering::Greater,
                    // Both have pre-release: compare pre-release strings
                    (Some(a), Some(b)) => a.cmp(b),
                }
            }
            non_eq => non_eq,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_versions() {
        let v = Version::parse("v1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert_eq!(v.pre, None);
        assert_eq!(v.to_string(), "1.2.3");

        let v2 = Version::parse("0.4.0-alpha").unwrap();
        assert_eq!(v2.major, 0);
        assert_eq!(v2.minor, 4);
        assert_eq!(v2.patch, 0);
        assert_eq!(v2.pre.as_deref(), Some("alpha"));
        assert!(v2.is_prerelease());
        assert_eq!(v2.to_string(), "0.4.0-alpha");
    }

    #[test]
    fn compares_correctly() {
        let v1 = Version::parse("0.3.0").unwrap();
        let v2 = Version::parse("0.4.0-alpha").unwrap();
        let v3 = Version::parse("0.4.0").unwrap();
        let v4 = Version::parse("1.0.0").unwrap();

        assert!(v1 < v2);
        assert!(v2 < v3);
        assert!(v3 < v4);
    }
}

use anyhow::{bail, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemVer {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl SemVer {
    pub fn parse(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 3 {
            bail!("invalid semver: expected MAJOR.MINOR.PATCH, got '{s}'");
        }
        Ok(SemVer {
            major: parts[0]
                .parse()
                .map_err(|_| anyhow::anyhow!("invalid major in '{s}'"))?,
            minor: parts[1]
                .parse()
                .map_err(|_| anyhow::anyhow!("invalid minor in '{s}'"))?,
            patch: parts[2]
                .parse()
                .map_err(|_| anyhow::anyhow!("invalid patch in '{s}'"))?,
        })
    }
}

impl std::fmt::Display for SemVer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

pub fn check_major_match(tool_version: &str, binary: &str) -> Result<()> {
    let tool = SemVer::parse(tool_version)?;
    let ours = SemVer::parse(env!("CARGO_PKG_VERSION"))?;

    if tool.major != ours.major {
        bail!(
            "protocol version mismatch: '{binary}' reports '{tool_version}', \
             attach-meta is '{}' — major version must match \
             (got {}, need {})",
            ours,
            tool.major,
            ours.major,
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid() {
        let v = SemVer::parse("1.2.3").unwrap();
        assert_eq!(
            v,
            SemVer {
                major: 1,
                minor: 2,
                patch: 3
            }
        );
    }

    #[test]
    fn parse_invalid() {
        assert!(SemVer::parse("1.2").is_err());
        assert!(SemVer::parse("abc").is_err());
        assert!(SemVer::parse("1.2.x").is_err());
    }

    #[test]
    fn major_match_ok() {
        assert!(check_major_match(env!("CARGO_PKG_VERSION"), "test").is_ok());
    }

    #[test]
    fn major_mismatch() {
        assert!(check_major_match("99.0.0", "test").is_err());
    }
}

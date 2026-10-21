//! Manifest v1 compatibility, independent of the Engine release version.
use semver::{Comparator, Op, Version, VersionReq};
use serde::Deserialize;

pub const ENGINE_API: &str = "0.1.0";
pub const CAPABILITIES: &[&str] = &[
    "dialogue.yarn/1",
    "dialogue.collections/1",
    "assets.catalog/1",
    "ui.screens/1",
    "presentation.media/1",
    "save/1",
    "rules.events/1",
    "rules.commands/1",
    "state.collections/1",
    "rules.rhai/1",
    "rules.random/1",
    "localization.catalog/1",
];

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Requirements {
    pub engine_api: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

/// Whitespace-separated explicit comparators, each with a complete SemVer; no implicit caret or wildcards.
pub fn api_matches(range: &str, api: &str) -> Result<bool, String> {
    let mut comparators = Vec::new();
    for part in range.split_whitespace() {
        let version = [">=", "<=", "=", ">", "<"].iter().find_map(|op| part.strip_prefix(op))
            .ok_or_else(|| format!("invalid Engine API range `{range}`: use explicit comparators such as `>=0.1.0 <0.2.0`"))?;
        Version::parse(version)
            .map_err(|e| format!("invalid Engine API comparator `{part}`: {e}"))?;
        let comparator = Comparator::parse(part)
            .map_err(|e| format!("invalid Engine API comparator `{part}`: {e}"))?;
        if !matches!(
            comparator.op,
            Op::Exact | Op::Greater | Op::GreaterEq | Op::Less | Op::LessEq
        ) {
            return Err(format!("invalid Engine API comparator `{part}`"));
        }
        comparators.push(comparator);
    }
    if comparators.is_empty() {
        return Err("Engine API range must not be empty".into());
    }
    let api =
        Version::parse(api).map_err(|e| format!("invalid Engine API version `{api}`: {e}"))?;
    Ok(VersionReq { comparators }.matches(&api))
}

pub fn check(
    requirements: &Requirements,
    api: &str,
    capabilities: &[&str],
) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    match api_matches(&requirements.engine_api, api) {
        Ok(true) => {}
        Ok(false) => errors.push(format!("requires Engine API `{}`, engine provides `{api}`; prerelease APIs require an explicit prerelease comparator", requirements.engine_api)),
        Err(error) => errors.push(error),
    }
    for required in &requirements.capabilities {
        if !capabilities.contains(&required.as_str()) {
            errors.push(format!(
                "requires capability `{required}`, engine provides {capabilities:?}"
            ));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn adr_phase_one_version_matrix() {
        for api in ["0.1.0", "0.1.1", "0.2.0"] {
            assert!(api_matches(">=0.1.0 <0.3.0", api).unwrap());
        }
        assert!(!api_matches(">=0.1.0 <0.3.0", "0.3.0").unwrap());
        assert!(!api_matches(">=0.1.0 <0.3.0", "0.2.0-beta.1").unwrap());
        assert!(api_matches(">=0.2.0-beta.1 <0.3.0", "0.2.0-beta.2").unwrap());
        for range in [
            "",
            "*",
            "^0.1.0",
            "0.1.0",
            ">=0.1",
            ">=0.1.0, <0.2.0",
            ">= 0.1.0",
        ] {
            assert!(api_matches(range, ENGINE_API).is_err(), "{range}");
        }
        let required = Requirements {
            engine_api: ">=0.1.0 <0.2.0".into(),
            capabilities: vec!["missing.feature/1".into()],
        };
        let errors = check(&required, ENGINE_API, CAPABILITIES).unwrap_err();
        assert!(errors[0].contains("missing.feature/1"));
        assert!(errors[0].contains("dialogue.yarn/1"));
    }
}

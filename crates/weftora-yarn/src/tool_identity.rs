//! Build-pair diagnostics; self-reported identity is not executable authentication.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolIdentity {
    pub identity_format: u32,
    pub tool: String,
    pub engine_release: String,
    pub build_id: String,
    pub target: String,
    pub profile: String,
    pub dialogue_artifact_format: u32,
    pub compiler_id: String,
    pub compiler_version: String,
}

impl ToolIdentity {
    pub fn current(tool: &str) -> Self {
        Self {
            identity_format: 1,
            tool: tool.into(),
            engine_release: env!("CARGO_PKG_VERSION").into(),
            build_id: env!("WEFTORA_TOOL_BUILD_ID").into(),
            target: env!("WEFTORA_TOOL_TARGET").into(),
            profile: env!("WEFTORA_TOOL_PROFILE").into(),
            dialogue_artifact_format: crate::ARTIFACT_FORMAT,
            compiler_id: crate::COMPILER_ID.into(),
            compiler_version: crate::COMPILER_VERSION.into(),
        }
    }
}

//! Bounded Story translation catalogs. Rendering remains the adapter's responsibility.
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

use serde::Deserialize;
use weftora_api::Diagnostic;

use crate::{MANIFEST_FILE, Manifest, jsonc, paths, resolve_in_root};

pub const TEXT_CAPABILITY: &str = "localization.text/1";

pub const CAPABILITY: &str = "localization.catalog/1";
pub const MAX_LANGUAGES: usize = 8;
pub const MAX_CATALOG_BYTES: usize = 1024 * 1024;
pub const MAX_ENTRIES: usize = 4096;
pub const MAX_TEXT_BYTES: usize = 4096;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Definition {
    #[serde(default)]
    pub state_key: Option<String>,
    #[serde(default)]
    pub font: Option<String>,
    pub base_language: String,
    pub languages: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Catalog {
    pub localization_schema: u32,
    pub ui: BTreeMap<String, String>,
    #[serde(default)]
    pub dialogue: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct Catalogs {
    pub definition: Definition,
    pub languages: BTreeMap<String, Catalog>,
}

pub(crate) fn load(root: &Path, manifest: &Manifest) -> Result<Option<Catalogs>, Vec<Diagnostic>> {
    let mut errors = Vec::new();
    let required = manifest
        .requires
        .as_ref()
        .is_some_and(|r| r.capabilities.iter().any(|c| c == CAPABILITY));
    let text_required = manifest
        .requires
        .as_ref()
        .is_some_and(|r| r.capabilities.iter().any(|c| c == TEXT_CAPABILITY));
    let Some(definition) = &manifest.localization else {
        return if required || text_required {
            Err(vec![Diagnostic::new(
                MANIFEST_FILE,
                None,
                "localization.catalog/1 requires localization",
            )])
        } else {
            Ok(None)
        };
    };
    if !required {
        errors.push(Diagnostic::new(
            MANIFEST_FILE,
            None,
            "localization requires localization.catalog/1",
        ));
    }
    if text_required && (definition.state_key.is_none() || definition.font.is_none()) {
        errors.push(Diagnostic::new(
            MANIFEST_FILE,
            None,
            "localization.text/1 requires localization.stateKey and localization.font",
        ));
    }
    if !text_required && (definition.state_key.is_some() || definition.font.is_some()) {
        errors.push(Diagnostic::new(
            MANIFEST_FILE,
            None,
            "localization.stateKey/font requires localization.text/1",
        ));
    }
    if let Some(key) = &definition.state_key {
        match manifest.state.get(key) {
            Some(entry) if entry.scope == weftora_core::Scope::Settings && matches!(&entry.default, weftora_api::Value::String(language) if definition.languages.contains_key(language)) => {}
            _ => errors.push(Diagnostic::new(MANIFEST_FILE, None, "localization.stateKey must name a declared Settings string with a supported language default")),
        }
        if let Some(value) = manifest.initial_state.get(key)
            && !matches!(value, weftora_api::Value::String(language) if definition.languages.contains_key(language))
        {
            errors.push(Diagnostic::new(
                MANIFEST_FILE,
                None,
                "localization.stateKey initialState must name a supported language",
            ));
        }
    }
    if definition.languages.is_empty() || definition.languages.len() > MAX_LANGUAGES {
        errors.push(Diagnostic::new(
            MANIFEST_FILE,
            None,
            format!("localization.languages must contain 1..={MAX_LANGUAGES} languages"),
        ));
        return Err(errors);
    }
    if !definition.languages.contains_key(&definition.base_language) {
        errors.push(Diagnostic::new(
            MANIFEST_FILE,
            None,
            "localization.baseLanguage must name a declared language",
        ));
    }
    let mut sources = BTreeSet::new();
    let mut languages = BTreeMap::new();
    for (language, file) in &definition.languages {
        if language.is_empty() || language.len() > 64 || !language.is_ascii() {
            errors.push(Diagnostic::new(
                MANIFEST_FILE,
                None,
                "localization language tags must be 1..=64 ASCII bytes",
            ));
        }
        if !sources.insert(file.to_ascii_lowercase()) {
            errors.push(Diagnostic::new(
                MANIFEST_FILE,
                None,
                format!("localization.languages.{language:?}: duplicate catalog path {file:?}"),
            ));
            continue;
        }
        let result = (|| {
            let text = paths::read_text(resolve_in_root(root, file).map_err(|e| e.message)?)?;
            if text.len() > MAX_CATALOG_BYTES {
                return Err(format!(
                    "localization catalog exceeds {MAX_CATALOG_BYTES} bytes"
                ));
            }
            let catalog: Catalog = jsonc::from_str(&text).map_err(|e| e.to_string())?;
            if catalog.localization_schema != 1 {
                return Err(format!(
                    "unsupported localizationSchema {} (supports 1)",
                    catalog.localization_schema
                ));
            }
            if catalog.ui.len() + catalog.dialogue.len() > MAX_ENTRIES {
                return Err(format!(
                    "localization catalog exceeds {MAX_ENTRIES} entries"
                ));
            }
            if language == &definition.base_language && !catalog.dialogue.is_empty() {
                return Err("base language dialogue must be empty; Yarn is the source text".into());
            }
            for (section, entries) in [("ui", &catalog.ui), ("dialogue", &catalog.dialogue)] {
                for (key, text) in entries {
                    let valid = if section == "dialogue" {
                        key.len() <= 256
                            && key.strip_prefix("line:").is_some_and(|id| !id.is_empty())
                            && !key.chars().any(|c| c.is_control() || c.is_whitespace())
                    } else {
                        crate::ui::valid_id(key)
                    };
                    if !valid {
                        return Err(format!(
                            "/{section}/{key:?}: invalid localization key (dialogue keys require line: plus a nonempty ID, at most 256 UTF-8 bytes total, without whitespace/controls)"
                        ));
                    }
                    if text.len() > MAX_TEXT_BYTES
                        || text
                            .chars()
                            .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
                    {
                        return Err(format!(
                            "/{section}/{key}: text must be at most {MAX_TEXT_BYTES} UTF-8 bytes, without controls other than newline/tab"
                        ));
                    }
                }
            }
            Ok(catalog)
        })();
        match result {
            Ok(catalog) => {
                languages.insert(language.clone(), catalog);
            }
            Err(message) => errors.push(Diagnostic::new(file, None, message)),
        }
    }
    if let Some(base) = languages.get(&definition.base_language) {
        for (language, catalog) in &languages {
            let file = &definition.languages[language];
            for key in base.ui.keys() {
                if !catalog.ui.contains_key(key) {
                    errors.push(Diagnostic::new(
                        file,
                        None,
                        format!("/ui/{key}: missing translation key"),
                    ));
                }
            }
            for key in catalog.ui.keys() {
                if !base.ui.contains_key(key) {
                    errors.push(Diagnostic::new(
                        file,
                        None,
                        format!("/ui/{key}: unknown translation key"),
                    ));
                }
            }
        }
    }
    if errors.is_empty() {
        Ok(Some(Catalogs {
            definition: definition.clone(),
            languages,
        }))
    } else {
        Err(errors)
    }
}

impl Catalogs {
    /// Read the configured Settings value without interpreting any hardcoded state key.
    pub fn language(&self, state: &weftora_core::State) -> Result<String, String> {
        let Some(key) = &self.definition.state_key else {
            return Ok(self.definition.base_language.clone());
        };
        match state.get(key).map_err(|e| e.to_string())? {
            weftora_api::Value::String(language) if self.languages.contains_key(language) => {
                Ok(language.clone())
            }
            _ => Err(format!(
                "localization state `{key}` must name a supported language"
            )),
        }
    }

    pub fn ui_text<'a>(
        &'a self,
        state: &weftora_core::State,
        key: &str,
    ) -> Result<&'a str, String> {
        let language = self.language(state)?;
        self.languages[&language]
            .ui
            .get(key)
            .map(String::as_str)
            .ok_or_else(|| format!("missing localization UI key `{key}` for `{language}`"))
    }

    /// Check against compiled Yarn templates, including option text. No compiler dependency is needed.
    pub fn dialogue_errors(&self, strings: &BTreeMap<String, String>) -> Vec<Diagnostic> {
        let mut errors = Vec::new();
        for (language, catalog) in &self.languages {
            if language == &self.definition.base_language {
                continue;
            }
            let file = &self.definition.languages[language];
            for (key, source) in strings {
                match catalog.dialogue.get(key) {
                    None => errors.push(Diagnostic::new(
                        file,
                        None,
                        format!("/dialogue/{key}: missing translation key"),
                    )),
                    Some(text) => match (placeholders(source), placeholders(text)) {
                        (Ok(a), Ok(b)) if a == b => {}
                        _ => errors.push(Diagnostic::new(
                            file,
                            None,
                            format!(
                                "/dialogue/{key}: placeholders must match source indices exactly"
                            ),
                        )),
                    },
                }
            }
            for key in catalog.dialogue.keys() {
                if !strings.contains_key(key) {
                    errors.push(Diagnostic::new(
                        file,
                        None,
                        format!("/dialogue/{key}: unknown translation key"),
                    ));
                }
            }
        }
        errors
    }
}

// Yarn expands canonical numeric tokens only; other braces remain literal text.
fn placeholders(text: &str) -> Result<BTreeSet<usize>, ()> {
    let mut indices = BTreeSet::new();
    for part in text.split('{').skip(1) {
        let Some((number, _)) = part.split_once('}') else {
            continue;
        };
        if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let index: usize = number.parse().map_err(|_| ())?;
        if index.to_string() != number {
            return Err(());
        }
        indices.insert(index);
    }
    Ok(indices)
}

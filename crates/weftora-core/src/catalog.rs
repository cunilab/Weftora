//! Stable ID lookup with source-aware diagnostics.
use std::collections::BTreeMap;
use weftora_api::Diagnostic;

#[derive(Debug, Clone)]
pub struct Catalog<T> {
    entries: BTreeMap<String, (T, String, String)>,
}
impl<T> Default for Catalog<T> {
    fn default() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }
}
impl<T> Catalog<T> {
    pub fn insert(
        &mut self,
        id: &str,
        value: T,
        file: &str,
        location: &str,
    ) -> Result<(), Diagnostic> {
        if id.trim().is_empty() {
            return Err(Diagnostic::new(
                file,
                None,
                format!("{location}: catalog ID must not be empty"),
            ));
        }
        if let Some((_, first_file, first_location)) = self.entries.get(id) {
            return Err(Diagnostic::new(
                file,
                None,
                format!(
                    "{location}: duplicate catalog ID `{id}`; first declared at {first_file}:{first_location}"
                ),
            ));
        }
        self.entries
            .insert(id.into(), (value, file.into(), location.into()));
        Ok(())
    }
    pub fn get(&self, id: &str, file: &str, location: &str) -> Result<&T, Diagnostic> {
        self.entries
            .get(id)
            .map(|(value, _, _)| value)
            .ok_or_else(|| {
                Diagnostic::new(file, None, format!("{location}: missing catalog ID `{id}`"))
            })
    }
    pub fn iter(&self) -> impl Iterator<Item = (&str, &T)> {
        self.entries
            .iter()
            .map(|(id, value)| (id.as_str(), &value.0))
    }
}

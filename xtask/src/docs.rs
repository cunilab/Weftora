use crate::{files, markdown_files};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::{Component, Path, PathBuf},
};

pub fn run(root: &Path) -> Result<(), String> {
    let mut documents = BTreeMap::new();
    for path in markdown_files(root)? {
        documents.insert(path, false);
    }
    for path in files(&root.join("docs"))?
        .into_iter()
        .chain([root.join("PRD.md"), root.join("ROADMAP.md")])
    {
        let path = path
            .strip_prefix(root)
            .map_err(|error| error.to_string())?
            .to_path_buf();
        if path.extension().is_some_and(|ext| ext == "md") && !path.starts_with("docs/templates") {
            documents.insert(path, true);
        }
    }
    let mut problems = Vec::new();
    let mut ids = HashMap::new();
    for (path, governed) in documents {
        let text = match std::fs::read_to_string(root.join(&path)) {
            Ok(text) => text,
            Err(error) => {
                problem(
                    &mut problems,
                    &path,
                    1,
                    format!("cannot read document: {error}"),
                );
                continue;
            }
        };
        if governed {
            check_header(&path, &text, &mut ids, &mut problems);
        }
        check_links(
            &path,
            &text,
            &|target| exists_case_sensitive(root, target),
            &|target| std::fs::read_to_string(root.join(target)).map_err(|error| error.to_string()),
            &mut problems,
        );
    }
    if problems.is_empty() {
        println!("documentation headers and links passed");
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}

/// Check each spelling before advancing, even on case-insensitive filesystems.
fn exists_case_sensitive(root: &Path, target: &Path) -> bool {
    let mut directory = root.to_path_buf();
    for component in target.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                directory.pop();
            }
            Component::Normal(name) => {
                let Ok(entries) = std::fs::read_dir(&directory) else {
                    return false;
                };
                if !entries
                    .filter_map(Result::ok)
                    .any(|entry| entry.file_name() == name)
                {
                    return false;
                }
                directory.push(name);
            }
            Component::RootDir | Component::Prefix(_) => return false,
        }
    }
    directory.exists()
}

fn problem(problems: &mut Vec<String>, path: &Path, line: usize, message: impl AsRef<str>) {
    problems.push(format!("{}:{line}: {}", path.display(), message.as_ref()));
}

fn valid_id(id: &str) -> bool {
    let parts: Vec<_> = id.split('-').collect();
    parts.len() == 3
        && parts[0] == "WFT"
        && !parts[1].is_empty()
        && parts[1].bytes().all(|byte| byte.is_ascii_uppercase())
        && parts[2].len() == 3
        && parts[2].bytes().all(|byte| byte.is_ascii_digit())
}

// These headers deliberately have a flat, three-string schema; no YAML library is needed.
fn scalar(value: &str) -> Option<String> {
    let value = value.trim();
    if value.starts_with('"') {
        serde_json::from_str::<String>(value).ok()
    } else if value.starts_with('\'') {
        value
            .strip_prefix('\'')?
            .strip_suffix('\'')
            .map(|text| text.replace("''", "'"))
    } else if value.is_empty()
        || value.starts_with(['[', '{', '|', '>', '&', '*', '!', '#'])
        || value.contains(": ")
    {
        None
    } else {
        Some(value.split(" #").next()?.trim_end().to_owned())
    }
}

fn check_header(
    path: &Path,
    text: &str,
    ids: &mut HashMap<String, PathBuf>,
    problems: &mut Vec<String>,
) {
    let lines: Vec<_> = text.lines().collect();
    if lines.first().copied() != Some("---") {
        problem(
            problems,
            path,
            1,
            "document must start with a YAML header (---)",
        );
        return;
    }
    let Some(end) = lines
        .iter()
        .enumerate()
        .skip(1)
        .find_map(|(i, line)| (*line == "---").then_some(i))
    else {
        problem(problems, path, 1, "YAML header has no closing ---");
        return;
    };
    let mut fields = HashMap::new();
    for (index, line) in lines.iter().enumerate().take(end).skip(1) {
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once(':') else {
            problem(problems, path, index + 1, "expected a YAML key: value");
            continue;
        };
        let key = key.trim();
        if !["id", "title", "status"].contains(&key) {
            problem(
                problems,
                path,
                index + 1,
                format!("unexpected header key {key:?}"),
            );
        }
        if line.starts_with(char::is_whitespace) {
            problem(
                problems,
                path,
                index + 1,
                "header keys must be at the top level",
            );
        }
        let Some(value) = scalar(value) else {
            problem(
                problems,
                path,
                index + 1,
                format!("{key} must be a YAML string"),
            );
            continue;
        };
        if fields.insert(key, (value, index + 1)).is_some() {
            problem(
                problems,
                path,
                index + 1,
                format!("duplicate header key {key}"),
            );
        }
    }
    for key in ["id", "title", "status"] {
        if !fields.contains_key(key) {
            problem(problems, path, 1, format!("missing header key {key}"));
        }
    }
    if let Some((id, line)) = fields.get("id") {
        if !valid_id(id) {
            problem(problems, path, *line, "id must match WFT-[A-Z]+-[0-9]{3}");
        }
        if let Some(previous) = ids.get(id) {
            problem(
                problems,
                path,
                *line,
                format!("duplicate id {id} (also in {})", previous.display()),
            );
        } else {
            ids.insert(id.clone(), path.to_path_buf());
        }
    }
    if let Some((status, line)) = fields.get("status")
        && !["draft", "proposed", "accepted", "superseded"].contains(&status.as_str())
    {
        problem(
            problems,
            path,
            *line,
            "status must be draft, proposed, accepted or superseded",
        );
    }
    if let Some((title, line)) = fields.get("title") {
        let headings = headings(text);
        match headings.iter().find(|(level, _)| *level == 1) {
            Some((_, heading)) if title == heading => {}
            Some((_, heading)) => problem(
                problems,
                path,
                *line,
                format!("title {title:?} does not match first H1 {heading:?}"),
            ),
            None => problem(problems, path, *line, "document has no H1 heading"),
        }
    }
}

/// Return numbered lines outside CommonMark-style backtick/tilde fences.
fn prose(text: &str) -> Vec<(usize, &str)> {
    let mut result = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        let first = trimmed.chars().next();
        let marker = first.filter(|ch| *ch == '`' || *ch == '~');
        let count = marker.map_or(0, |ch| {
            trimmed.chars().take_while(|next| *next == ch).count()
        });
        if let Some((ch, length)) = fence {
            if marker == Some(ch) && count >= length && trimmed[count..].trim().is_empty() {
                fence = None;
            }
        } else if count >= 3 {
            fence = Some((marker.unwrap(), count));
        } else {
            result.push((index + 1, line));
        }
    }
    result
}

fn headings(text: &str) -> Vec<(usize, String)> {
    let header_end = if text.lines().next() == Some("---") {
        text.lines()
            .enumerate()
            .skip(1)
            .find_map(|(index, line)| (line == "---").then_some(index + 1))
            .unwrap_or(text.lines().count())
    } else {
        0
    };
    let lines: Vec<_> = prose(text)
        .into_iter()
        .filter(|(line, _)| *line > header_end)
        .collect();
    let mut result = Vec::new();
    for (index, (_, line)) in lines.iter().enumerate() {
        let trimmed = line.trim_start();
        let hashes = trimmed.bytes().take_while(|byte| *byte == b'#').count();
        if (1..=6).contains(&hashes)
            && (trimmed.len() == hashes || trimmed.as_bytes()[hashes].is_ascii_whitespace())
        {
            let heading = trimmed[hashes..].trim();
            let without_hashes = heading.trim_end_matches('#');
            let heading =
                if without_hashes.ends_with(char::is_whitespace) || without_hashes.is_empty() {
                    without_hashes.trim_end()
                } else {
                    heading
                };
            result.push((hashes, heading.to_owned()));
        } else if index > 0
            && !trimmed.is_empty()
            && (trimmed.chars().all(|ch| ch == '=') || trimmed.chars().all(|ch| ch == '-'))
            && lines[index - 1].0 + 1 == lines[index].0
            && !lines[index - 1].1.trim().is_empty()
        {
            result.push((
                if trimmed.starts_with('=') { 1 } else { 2 },
                lines[index - 1].1.trim().to_owned(),
            ));
        }
    }
    result
}

fn slugs(text: &str) -> HashSet<String> {
    let mut result = HashSet::new();
    for (_, heading) in headings(text) {
        let base: String = heading_link_text(&heading)
            .to_lowercase()
            .chars()
            .filter(|ch| ch.is_alphanumeric() || matches!(ch, ' ' | '-' | '_'))
            .map(|ch| if ch == ' ' { '-' } else { ch })
            .collect();
        let mut slug = base.clone();
        let mut duplicate = 0;
        while result.contains(&slug) {
            duplicate += 1;
            slug = format!("{base}-{duplicate}");
        }
        result.insert(slug);
    }
    result
}

/// Render inline link labels without including destinations in heading slugs.
fn heading_link_text(heading: &str) -> String {
    let mut result = String::new();
    let mut remaining = heading;
    while let Some(start) = remaining.find('[') {
        result.push_str(&remaining[..start]);
        remaining = &remaining[start..];
        if let Some(label_end) = remaining.find("](")
            && let Some(end) = closing_parenthesis(&remaining[label_end + 2..])
        {
            result.push_str(&remaining[1..label_end]);
            remaining = &remaining[label_end + 2 + end + 1..];
        } else {
            result.push('[');
            remaining = &remaining[1..];
        }
    }
    result.push_str(remaining);
    result
}

/// Equal-length backtick runs delimit code spans; unmatched runs remain literal.
/// Preserve newlines so diagnostics still refer to the original source lines.
fn strip_inline_code(text: &str) -> String {
    let mut result = String::new();
    let mut remaining = text;
    while let Some(start) = remaining.find('`') {
        result.push_str(&remaining[..start]);
        remaining = &remaining[start..];
        let length = remaining.bytes().take_while(|byte| *byte == b'`').count();
        let mut offset = length;
        let mut end = None;
        while let Some(next) = remaining[offset..].find('`') {
            let next = offset + next;
            let run = remaining[next..]
                .bytes()
                .take_while(|byte| *byte == b'`')
                .count();
            if run == length {
                end = Some(next + run);
                break;
            }
            offset = next + run;
        }
        if let Some(end) = end {
            result.extend(
                remaining[..end]
                    .chars()
                    .map(|ch| if ch == '\n' { '\n' } else { ' ' }),
            );
            remaining = &remaining[end..];
        } else {
            result.push_str(&remaining[..length]);
            remaining = &remaining[length..];
        }
    }
    result.push_str(remaining);
    result
}

fn closing_parenthesis(text: &str) -> Option<usize> {
    let mut depth = 1;
    let mut escaped = false;
    for (index, ch) in text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn decode(value: &str) -> Result<String, String> {
    let mut bytes = Vec::new();
    let mut source = value.bytes();
    while let Some(byte) = source.next() {
        if byte == b'%' {
            let hex = [
                source.next().ok_or("incomplete percent escape")?,
                source.next().ok_or("incomplete percent escape")?,
            ];
            let hex = std::str::from_utf8(&hex).map_err(|error| error.to_string())?;
            bytes.push(u8::from_str_radix(hex, 16).map_err(|error| error.to_string())?);
        } else {
            bytes.push(byte);
        }
    }
    String::from_utf8(bytes).map_err(|error| error.to_string())
}

fn destinations(line: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut remaining = line;
    while let Some(start) = remaining.find("](") {
        remaining = &remaining[start + 2..];
        let Some(end) = closing_parenthesis(remaining) else {
            break;
        };
        let destination = remaining[..end].trim();
        if let Some(angle) = destination.strip_prefix('<') {
            if let Some(end) = angle.find('>') {
                result.push(&angle[..end]);
            }
        } else {
            result.push(destination.split_whitespace().next().unwrap_or(""));
        }
        remaining = &remaining[end + 1..];
    }
    result
}

fn check_links(
    path: &Path,
    text: &str,
    exists: &impl Fn(&Path) -> bool,
    read: &impl Fn(&Path) -> Result<String, String>,
    problems: &mut Vec<String>,
) {
    let mut lines = vec![""; text.lines().count()];
    for (line, content) in prose(text) {
        lines[line - 1] = content;
    }
    let text = strip_inline_code(&lines.join("\n"));
    for (index, prose) in text.lines().enumerate() {
        let line = index + 1;
        for destination in destinations(prose) {
            if ["http://", "https://", "mailto:"]
                .iter()
                .any(|prefix| destination.starts_with(prefix))
            {
                continue;
            }
            let (file, anchor) = destination
                .split_once('#')
                .map_or((destination, None), |(file, anchor)| (file, Some(anchor)));
            let file = file.split_once('?').map_or(file, |(file, _)| file);
            let file = match decode(file) {
                Ok(file) => file,
                Err(error) => {
                    problem(
                        problems,
                        path,
                        line,
                        format!("invalid link {destination:?}: {error}"),
                    );
                    continue;
                }
            };
            let target = if file.is_empty() {
                path.to_path_buf()
            } else if let Some(file) = file.strip_prefix('/') {
                PathBuf::from(file)
            } else {
                path.parent().unwrap_or(Path::new("")).join(file)
            };
            if !exists(&target) {
                problem(
                    problems,
                    path,
                    line,
                    format!(
                        "broken link {destination:?}: {} does not exist with this exact case",
                        target.display()
                    ),
                );
                continue;
            }
            if let Some(anchor) = anchor.filter(|anchor| !anchor.is_empty()) {
                match decode(anchor).and_then(|anchor| read(&target).map(|text| (anchor, text))) {
                    Ok((anchor, text)) if slugs(&text).contains(&anchor) => {}
                    Ok((anchor, _)) => problem(
                        problems,
                        path,
                        line,
                        format!("broken anchor #{anchor} in {}", target.display()),
                    ),
                    Err(error) => problem(
                        problems,
                        path,
                        line,
                        format!("cannot check anchor in {}: {error}", target.display()),
                    ),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const GOOD: &str = "---\nid: WFT-TEST-001\ntitle: Test\nstatus: draft\n---\n\n# Test\n";
    fn headers(text: &str) -> Vec<String> {
        let mut problems = Vec::new();
        check_header(
            Path::new("docs/test.md"),
            text,
            &mut HashMap::new(),
            &mut problems,
        );
        problems
    }
    fn links(text: &str) -> Vec<String> {
        let mut problems = Vec::new();
        check_links(
            Path::new("README.md"),
            text,
            &|path| path == Path::new("target.md"),
            &|_| Ok("# A Heading!\n## A Heading!\n".into()),
            &mut problems,
        );
        problems
    }
    #[test]
    fn valid_header() {
        assert!(headers(GOOD).is_empty());
    }
    #[test]
    fn bad_header() {
        assert!(headers("# Test")[0].contains("start with a YAML header"));
        let problems =
            headers(&GOOD.replace("status: draft", "status: invalid\nextra: field\nid: nope"));
        assert!(problems.iter().any(|p| p.contains("unexpected header key")));
        assert!(problems.iter().any(|p| p.contains("duplicate header key")));
        assert!(problems.iter().any(|p| p.contains("id must match")));
        assert!(problems.iter().any(|p| p.contains("status must be")));
    }
    #[test]
    fn duplicate_id() {
        let mut ids = HashMap::new();
        let mut problems = Vec::new();
        check_header(Path::new("docs/a.md"), GOOD, &mut ids, &mut problems);
        check_header(Path::new("docs/b.md"), GOOD, &mut ids, &mut problems);
        assert!(problems[0].contains("docs/b.md:2: duplicate id"));
    }
    #[test]
    fn title_mismatch() {
        assert!(
            headers(&GOOD.replace("title: Test", "title: Other"))[0]
                .contains("does not match first H1")
        );
    }
    #[test]
    fn broken_link() {
        assert!(links("[link](missing.md)")[0].contains("README.md:1: broken link"));
    }
    #[test]
    fn broken_anchor() {
        assert!(links("[link](target.md#missing)")[0].contains("broken anchor"));
    }
    #[test]
    fn valid_anchors_and_external_links() {
        assert!(links("[a](target.md#a-heading) [b](target.md#a-heading-1) [c](https://example.com) [d](mailto:a@example.com)").is_empty());
    }
    #[test]
    fn fences_ignored() {
        assert!(links("````md\n[bad](missing.md)\n```\n[bad](missing.md)\n````\n~~~\n[bad](missing.md)\n~~~").is_empty());
        assert_eq!(
            links("```\n[bad](missing.md)\n```\n[bad](missing.md)").len(),
            1
        );
    }

    #[test]
    fn link_paths_require_exact_case_for_every_component() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert!(exists_case_sensitive(root, Path::new("src/docs.rs")));
        assert!(exists_case_sensitive(root, Path::new("src/../src/docs.rs")));
        for path in ["Src/docs.rs", "src/Docs.rs", "Src/../src/docs.rs"] {
            assert!(
                !exists_case_sensitive(root, Path::new(path)),
                "accepted {path}"
            );
        }
    }

    #[test]
    fn inline_code_links_ignored() {
        assert!(
            links("`[bad](missing.md)` ``[bad](missing.md) ` nested`` [ok](target.md)").is_empty()
        );
        assert!(links("``[bad](missing.md)\n[bad](missing.md)``").is_empty());
        let problems = links("`[bad](missing.md)`\n[bad](missing.md)");
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("README.md:2:"));
        assert_eq!(links("` unmatched [bad](missing.md)").len(), 1);
    }

    #[test]
    fn deeply_indented_list_fences_ignored() {
        let text = "- list\n        ````md\n        [bad](missing.md)\n        ```\n        ~~~~\n        [bad](missing.md)\n        ````\n        ~~~md\n        [bad](missing.md)\n        ~~~\n[ok](target.md)";
        assert!(links(text).is_empty());
        assert_eq!(links(&format!("{text}\n[bad](missing.md)")).len(), 1);
    }

    #[test]
    fn front_matter_is_not_a_heading() {
        let text = "---\n# Fake H1\ntitle: Actual\n===\n---\n\n# Actual\n";
        assert_eq!(headings(text), vec![(1, "Actual".into())]);
        assert_eq!(slugs(text), HashSet::from(["actual".into()]));
    }

    #[test]
    fn heading_link_labels_define_slugs() {
        let text = "# Read [the guide](guide.md) now\n## [A Heading!](file(with-parentheses).md)\n";
        assert_eq!(
            slugs(text),
            HashSet::from(["read-the-guide-now".into(), "a-heading".into()])
        );
        let mut problems = Vec::new();
        check_links(
            Path::new("README.md"),
            "[a](target.md#read-the-guide-now)",
            &|_| true,
            &|_| Ok(text.into()),
            &mut problems,
        );
        assert!(problems.is_empty());
    }

    #[test]
    fn query_strings_and_repo_root_links_resolve() {
        assert!(links("[a](target.md?raw=1#a-heading) [b](target.md?raw=1)").is_empty());
        let mut problems = Vec::new();
        check_links(
            Path::new("docs/nested/source.md"),
            "[a](/target.md?raw=1#a-heading) [b](/target.md)",
            &|path| path == Path::new("target.md"),
            &|_| Ok("# A Heading!".into()),
            &mut problems,
        );
        assert!(problems.is_empty());
        assert!(links("[bad](missing.md?raw=1)")[0].contains("broken link"));
        assert!(links("[bad](/target.md?raw=1#missing)")[0].contains("broken anchor"));
    }
}

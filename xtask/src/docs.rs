use crate::{command_output, files};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::{Path, PathBuf},
    process::Command,
};

pub fn run(root: &Path) -> Result<(), String> {
    let tracked = command_output(
        Command::new("git")
            .current_dir(root)
            .args(["ls-files", "-z"]),
    )?;
    let mut documents = BTreeMap::new();
    for name in tracked
        .split(|byte| *byte == 0)
        .filter(|name| !name.is_empty())
    {
        let name = std::str::from_utf8(name).map_err(|error| error.to_string())?;
        let path = PathBuf::from(name);
        if path.extension().is_some_and(|ext| ext == "md") {
            documents.insert(path, false);
        }
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
            &|target| root.join(target).exists(),
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
        let indent = line.len() - trimmed.len();
        let first = trimmed.chars().next();
        let marker = first.filter(|ch| *ch == '`' || *ch == '~');
        let count = marker.map_or(0, |ch| {
            trimmed.chars().take_while(|next| *next == ch).count()
        });
        if let Some((ch, length)) = fence {
            if indent <= 3
                && marker == Some(ch)
                && count >= length
                && trimmed[count..].trim().is_empty()
            {
                fence = None;
            }
        } else if indent <= 3 && count >= 3 {
            fence = Some((marker.unwrap(), count));
        } else {
            result.push((index + 1, line));
        }
    }
    result
}

fn headings(text: &str) -> Vec<(usize, String)> {
    let lines = prose(text);
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
        let base: String = heading
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
        let mut depth = 1;
        let mut escaped = false;
        let mut end = None;
        for (i, ch) in remaining.char_indices() {
            if escaped {
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == '(' {
                depth += 1;
            }
            if ch == ')' {
                depth -= 1;
                if depth == 0 {
                    end = Some(i);
                    break;
                }
            }
        }
        let Some(end) = end else {
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
    for (line, prose) in prose(text) {
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
            } else {
                path.parent().unwrap_or(Path::new("")).join(file)
            };
            if !exists(&target) {
                problem(
                    problems,
                    path,
                    line,
                    format!(
                        "broken link {destination:?}: {} does not exist",
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
}

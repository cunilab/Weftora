"""Check governed markdown docs, metadata, local links, index. Stdlib only."""
import datetime
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
DOCS = [ROOT / "PRD.md", ROOT / "ROADMAP.md", *sorted((ROOT / "docs").rglob("*.md"))]
GOVERNED = [p for p in DOCS if "templates" not in p.parts]
KEYS = set("id title type doc_version status implementation created updated last_reviewed owner scope product_release related_pr supersedes".split())
TYPES = set("product_requirements roadmap architecture platform_plan acceptance_criteria docs_index documentation_standard architecture_decision".split())
STATES = set("draft proposed accepted superseded".split())
IMPL = set("not_started in_progress verified not_applicable".split())
SCOPES = set("windows-first multiplatform-planning documentation".split())
errors = []


def fail(path, message):
    errors.append(f"{path.relative_to(ROOT)}: {message}")


def frontmatter(path):
    text = path.read_text(encoding="utf-8")
    match = re.match(r"\A---\n(.*?)\n---\n", text, re.S)
    if not match:
        fail(path, "missing frontmatter")
        return {}
    info = {}
    for line in match.group(1).splitlines():
        if not re.fullmatch(r"[a-z_]+:.*", line):
            fail(path, "bad metadata line " + line)
            continue
        key, value = line.split(":", 1)
        if key in info:
            fail(path, "duplicate key " + key)
        info[key] = value.strip().strip('"').strip("'")
    if set(info) != KEYS:
        fail(path, "required metadata keys differ")
    for name, allowed in (("type", TYPES), ("status", STATES),
                          ("implementation", IMPL), ("scope", SCOPES)):
        if info.get(name) not in allowed:
            fail(path, "invalid " + name)
    if not re.fullmatch(r"WFT-[A-Z]+-\d{3}", info.get("id", "")):
        fail(path, "invalid ID")
    if not re.fullmatch(r"\d+\.\d+\.\d+", info.get("doc_version", "")):
        fail(path, "invalid doc_version")
    for name in ("created", "updated", "last_reviewed"):
        value = info.get(name, "null")
        if value != "null":
            try:
                if not re.fullmatch(r"\d{4}-\d{2}-\d{2}", value):
                    raise ValueError()
                datetime.date.fromisoformat(value)
            except ValueError:
                fail(path, "invalid " + name)
    if info.get("created", "") > info.get("updated", ""):
        fail(path, "created after updated")
    if info.get("related_pr", "null") != "null" and not info["related_pr"].isdigit():
        fail(path, "related_pr invalid")
    if "## Change History" not in text:
        fail(path, "missing history")
    return info


def slug(s):
    return re.sub(r"\s", "-", re.sub(r"[^\w\s-]", "", s.lower()).strip())


def links(path):
    source = path.read_text(encoding="utf-8")
    for url in re.findall(r"(?<!!)\[[^\]]*\]\(([^)]+)\)", source):
        if re.match(r"(https?://|mailto:)", url):
            continue
        dest, sep, fragment = url.partition("#")
        target = (path.parent / dest).resolve() if dest else path
        if not target.is_relative_to(ROOT) or not target.is_file():
            fail(path, "broken link " + url)
        elif sep:
            headings = {slug(h) for h in re.findall(
                r"^#{1,6} +(.+)$", target.read_text(encoding="utf-8"), re.M)}
            if fragment not in headings:
                fail(path, "broken anchor " + url)


def main():
    info = {}
    ids = set()
    for path in GOVERNED:
        meta = frontmatter(path)
        if meta.get("id") in ids:
            fail(path, "duplicate ID")
        ids.add(meta.get("id"))
        info[path.resolve()] = meta
    for path in [ROOT / "README.md", *DOCS]:
        links(path)
    index = ROOT / "docs" / "README.md"
    present = set()
    pattern = re.compile(r"\| \[[^\]]+\]\(([^)]+\.md)\) \| (WFT-[A-Z]+-\d{3}) \| ([\d.]+) \| ([a-z_]+) \| ([a-z_]+) \| (\d{4}-\d{2}-\d{2}) \| ([a-z-]+) \|")
    for line in index.read_text(encoding="utf-8").splitlines():
        m = pattern.match(line)
        if not m:
            continue
        rel, *vals = m.groups()
        target = (index.parent / rel).resolve()
        present.add(target)
        actual = info.get(target, {})
        for key, val in zip(("id", "doc_version", "status", "implementation", "updated", "scope"), vals):
            if actual.get(key) != val:
                fail(index, f"{rel}: index mismatch: {key}")
    expected = set(info) - {index.resolve()}
    if present != expected:
        fail(index, "index entries differ from governed docs")
    for e in errors:
        print("ERROR:", e, file=sys.stderr)
    print(f"Docs checked: {len(GOVERNED)}; errors: {len(errors)}")
    return bool(errors)


if __name__ == "__main__":
    sys.exit(main())

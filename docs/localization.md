---
id: WFT-LOC-001
title: Localisation catalogs
status: proposed
---

# Localisation catalogs

The first localisation step validates translation catalogs. It does **not** switch the live player language or bind translated UI text yet. Preview and headless execution still use the Yarn source text. [P6-11](./success-criteria.md#phase-6--windows-foundation-release) remains open for switching, Japanese font integration and Windows release-build screenshots.

The [fixture Story](../examples/localization-demo/README.md) contains English (`en`, source), Indonesian (`id`) and Japanese (`ja`). Other Stories choose their languages; the Engine has no hardcoded language list. Translation quality requires human review. Coverage checks require keys, while allowing intentionally empty strings; they do not prove a translation is useful.

## Manifest and catalog format

Use manifest schema 1 and declare `localization.catalog/1` in `requires.capabilities` alongside `dialogue.yarn/1`. The capability requires a `localization` block, and the block requires the capability:

```json
"localization": {
  "baseLanguage": "en",
  "languages": {
    "en": "locales/en.jsonc",
    "id": "locales/id.jsonc",
    "ja": "locales/ja.jsonc"
  }
}
```

Language keys must be canonical BCP 47 language identifiers supported by the runtime's ICU parser, such as `en`, `id`, `ja-JP` or `zh-Hant`. Incorrect casing, malformed tags and extensions outside that parser's language-identifier format refuse validation. `baseLanguage` must be one of those keys. Catalog paths follow the ordinary Story path policy; two languages cannot share a path, including case-only aliases.

Every catalog is strict JSONC with schema 1 and a required `ui` object. Unknown fields and duplicate object keys are errors. The base catalog defines UI keys; every other catalog must have exactly those keys. UI values are literal text, with no interpolation in this step:

```json
{
  "localizationSchema": 1,
  "ui": {"continue": "Continue"}
}
```

The base catalog's optional `dialogue` object must be empty: Yarn is the single source of base dialogue. Every other catalog must translate every compiled string ID, including options and unreachable branches, with no extra IDs:

```json
{
  "localizationSchema": 1,
  "ui": {"continue": "続ける"},
  "dialogue": {
    "line:greeting": "ミラ: {0}さん、こんにちは。コインは{1}枚です。"
  }
}
```

Use explicit tags such as `#line:greeting` in Yarn to keep catalog IDs stable through source edits. Generated IDs are currently accepted, but source edits can change them and require catalog updates. The fixture uses explicit tags on every line and option. This capability does not introduce shadow-line alias resolution.

Dialogue substitutions use the compiler's canonical numeric tokens (`{0}`, `{1}`, …), not Yarn variable names. A translation must preserve the source's set of indices; reordering and repetition are allowed. Extra, missing, overflowing and noncanonical numeric indices such as `{00}` refuse validation. Other braces remain literal, matching the runtime's substitution behavior. UI placeholder parsing, translated markup parsing and locale-specific number formatting are outside this catalog-only step.

## Bounds and validation

| Item | Limit |
| --- | --- |
| Languages | 1–8 |
| Language key | 1–64 ASCII bytes, canonical ICU language identifier |
| Catalog file | 1 MiB of UTF-8 JSONC |
| Entries | 4,096 total across `ui` and `dialogue`, per catalog |
| UI key | 1–64 ASCII bytes, lowercase letters/digits/`_`/`.`/`-`, first character a letter or digit |
| Dialogue key | `line:` plus a nonempty Yarn ID, at most 256 UTF-8 bytes total, without whitespace/controls; case is preserved |
| Text value | 4,096 UTF-8 bytes; newline and tab are the only permitted controls |

Ordinary package file/count/total-size limits also apply. JSONC comments count toward the catalog file limit. Text limits count bytes, including multibyte Japanese characters.

`weftora check`, preview/headless `run` and `export` share validation. Missing keys, unknown keys and substitution mismatches produce file/key diagnostics and a non-zero exit. Invalid packages do not activate or create an export directory. Catalog shape/UI coverage are checked by the Story loader; language identifiers and dialogue coverage are checked against compiled strings by `weftora_yarn::check_story`. The compiler-free player uses that same check for artifacts.

Catalog files are ordinary Story content included in export and its content hashes. Translation edits do not change the compiled Yarn artifact or its source hash. This step adds no save fields, language setting or dialogue artifact format revision. Older engines without these manifest declarations refuse the Story; the capability identifies the additional catalog contract.

## Next implementation and native acceptance

The next step is immediate UI/dialogue/choice retranslation from captured substitutions and offered choices, preserving progress without replaying commands or re-evaluating choice conditions. It also needs a bundled Japanese font and its licence. Native release-build rendering, input, focus and screenshots remain queued through [issue #5](https://github.com/cunilab/Weftora/issues/5); these do not block portable implementation.

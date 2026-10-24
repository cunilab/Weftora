---
id: WFT-LOC-001
title: Localisation catalogs
status: proposed
---

# Localisation catalogs

Stories can validate catalogs and opt into live language switching for dialogue, choices and UI labels. The [three-language fixture](../examples/localization-demo/README.md) includes a licensed Japanese font. [P6-11](./success-criteria.md#phase-6--windows-foundation-release) remains open for Windows release-build visual/input acceptance and screenshots.

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

Every catalog is strict JSONC with schema 1 and a required `ui` object. Unknown fields and duplicate object keys are errors. The base catalog defines UI keys; every other catalog must have exactly those keys. UI values are literal text, with no interpolation:

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

Dialogue substitutions use the compiler's canonical numeric tokens (`{0}`, `{1}`, …), not Yarn variable names. A translation must preserve the source's set of indices; reordering and repetition are allowed. Extra, missing, overflowing and noncanonical numeric indices such as `{00}` refuse validation. Other braces remain literal, matching the runtime's substitution behavior. UI interpolation and locale-specific number formatting are not implemented. Runtime-enabled catalogs also receive syntactic markup validation using inert substitution values; this does not evaluate value-dependent `select`/`plural`/`ordinal` processors or prove every runtime value/branch. Dynamic markup errors retain the ordinary terminal Story error behavior.

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

Catalog files are ordinary Story content included in export and its content hashes. Translation edits do not change the compiled Yarn artifact or its source hash. Catalog validation alone adds no language setting. Runtime switching uses the configured canonical Settings key described below; neither capability adds save fields or a dialogue artifact format revision. Older engines without these manifest declarations refuse the Story; the capability identifies the additional catalog contract.

## Live language switching

Add `localization.text/1`, `ui.screens/1` and `assets.catalog/1` to the capabilities, keeping `localization.catalog/1`. Declare a Settings string and a common typed font asset:

```json
"state": {"ui.language": {"scope": "settings", "default": "en"}},
"localization": {
  "baseLanguage": "en",
  "stateKey": "ui.language",
  "font": "font.noto-jp",
  "languages": {"en": "locales/en.jsonc", "id": "locales/id.jsonc", "ja": "locales/ja.jsonc"}
}
```

The font ID must resolve through a typed font catalog. The Engine loads it within the existing 64 MiB UI preload budget and uses it for default dialogue/choices and declarative text/buttons/choice lists without an explicit font. Author font overrides still take precedence; authors must ensure they support their translations and substituted text. The fixture bundles the complete, unmodified [Noto Sans JP font and OFL notice](../examples/localization-demo/fonts/README.md), with pinned provenance and hashes. No network font lookup or host font is required for its Japanese text. The renderer enables Bevy's existing `complex_script_segmentation` feature so Japanese word/line boundaries use the bundled ICU dictionary rather than the non-complex-script fallback.

Bind a text or button using `"bind": {"localized": "continue"}`. The key must exist in every UI catalog. A widget still has exactly one of literal `text` or `bind`. Select a language with the ordinary UI action:

```json
{"do": "state.set", "key": "ui.language", "value": "ja"}
```

The key name is author-defined. Both its declared default and any initial override must name a configured language. Authored buttons are validated before activation. Unsupported developer edits fail preflight without changing live state; invalid canonical values supplied through a host fail safely rather than silently mixing languages.

Language is presentation-only. Yarn cannot read or write the configured locale variable; rule conditions/actions and Rhai read/write permissions cannot access it. Rules cannot subscribe to generic `state.changed` or to `state.changed.<localeKey>` in a runtime-localised Story. Use explicit notifications or keyed notifications for other mechanics. These restrictions preserve checkpoint replay independently of the current language. UI `state.set` remains the language selection path.

Switching re-renders the current line or offered choices from their IDs and already evaluated substitutions. It does not continue the VM, repeat commands/rewards, re-evaluate conditions, change visits or disarm a valid checkpoint. Choice IDs, order, availability and focused entities are retained. Queued snapshots translate when emitted. Normal progress, restart, selection and terminal failure clear stale snapshots. UI labels and choice text update in place; the fallback dialogue panel does too.

Captured values retain their original invariant string formatting; plural/ordinal markup resolves under the selected ICU language. Each captured line is bounded to 64 substitutions and 16 KiB aggregate substitution bytes, including unused values in crafted input, alongside existing expanded-text/option limits.

The locale remains in canonical Settings, outside progress saves. Loading progress retains the current in-process language and reconstructs the checkpoint in that language. A fresh launch uses the Story's initial/default locale; this feature does not add a persisted Settings file or automatic OS-language detection. Engine chrome may remain English. Translations and font changes are Story content and do not themselves alter the save schema.

## Native Windows acceptance

Windows release-verification CI also packages `localization-demo` with the same ZIP/extraction validation used for Stories A/B. Its verification artifact includes the game, compiled dialogue, Japanese font and licence; no Rust installation or font download is needed to run the extracted game. This is a review candidate, not a published release.

Use that release build and the fixture's three language buttons. Capture each language at the greeting and at choices, then switch repeatedly while a keyboard-focused choice is selected. Check immediate UI/dialogue/choice updates, Japanese glyphs, focus, resize/DPI and mouse/keyboard input. Save at the greeting in one language, switch to another and load: the greeting must retain the newly selected language and no progress or reward may repeat. Verify exported operation without system-installed Japanese fonts or network access.

Record OS/GPU/DPI, screenshots and observations in [issue #5](https://github.com/cunilab/Weftora/issues/5). Automated parser, glyph-coverage and model tests do not replace this native acceptance. P6-11 remains unchecked until that evidence passes.

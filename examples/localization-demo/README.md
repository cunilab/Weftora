# Three-language localisation fixture

English (`en`), Indonesian (`id`) and Japanese (`ja`) UI, dialogue and choices using the [localisation format](../../docs/localization.md). English dialogue comes from `main.yarn`; each line and choice has an explicit `#line:` tag. The common [Noto Sans JP font and OFL notice](./fonts/README.md) ship with the Story.

```sh
weftora check examples/localization-demo
weftora run examples/localization-demo
weftora run examples/localization-demo --headless --choices 0
weftora run examples/localization-demo --headless --choices 1
```

Select English, Bahasa Indonesia or 日本語 at the greeting or while choices are visible. UI labels, dialogue and offered choice text update immediately while progress and choice conditions stay fixed. The configured `ui.language` Settings key starts at `en`; a fresh launch uses that default, while loading progress retains the current in-process language. Save at the first greeting with F5, switch language, then load with F9. Developer node previews retain their existing save/load refusal.

The fixture has a source-language checkpoint and two finite branches. For headless language testing, change `initialState["ui.language"]` to one of `en`, `id`, `ja`; no Engine rebuild is needed. Translations are examples pending fluent-speaker review. Native [Windows acceptance](../../docs/localization.md#native-windows-acceptance), including font rendering, focus and screenshots, remains in [issue #5](https://github.com/cunilab/Weftora/issues/5).

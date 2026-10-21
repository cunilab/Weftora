# Three-language catalog fixture

English (`en`), Indonesian (`id`) and Japanese (`ja`) catalogs for the [localisation format](../../docs/localization.md). English dialogue comes from `main.yarn`; each line and choice has an explicit `#line:` tag. Translations preserve the two evaluated substitution slots in the greeting.

```sh
weftora check examples/localization-demo
weftora run examples/localization-demo --headless --choices 0
weftora run examples/localization-demo --headless --choices 1
```

This fixture currently proves catalog validation and both base-language branches. It does not yet demonstrate live language switching, translated UI bindings or Japanese font rendering. Those are the next P6-11 implementation steps. Translations are examples pending fluent-speaker review; Windows native acceptance remains in [issue #5](https://github.com/cunilab/Weftora/issues/5).

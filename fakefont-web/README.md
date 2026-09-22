# fakefont-web

`wasm-bindgen` bindings for [`fakefont`](../src/lib.rs), used by the
["How Big Is A Font?"](../web/) page.

The crate is a thin adapter — it does not add font logic. All it does is map the
library's Rust API onto the JavaScript API the page expects, rename a few
snake_case methods to camelCase, and convert library errors into real JavaScript
`Error` objects.

## JavaScript API

```js
import init, { FakeFont, tableStats } from "./pkg/fakefont_web.js";

await init(); // instantiate the module (installs a panic hook)

const font = new FakeFont("core", /* greek */ false, /* cyrillic */ true);

font.addDevanagari(); // optional, merges glyphs/kerning/features
font.addStandardArabic(); // the Arabic kernel subset
font.addUrduAndFarsi(); // the whole Naskh Arabic font
font.addBengali();
font.addThai();
font.addWeightAxis(100, 700); // omit for no wght axis
font.addWidthAxis(75, 125); // omit for no wdth axis
font.addArbitraryAxis("slnt", "Slant", -15, 15, 0, true, true); // tag, name, low, high, default, …
font.fillOutMasters(/* adjust kerning */ true, /* adjust advance widths */ true);

const bytes = font.compile(); // Uint8Array
const tables = tableStats(bytes); // Map<string, number>
```

| Rust                       | JavaScript                             |
| -------------------------- | -------------------------------------- |
| `FakeFont::new(coverage, greek, cyrillic)` | `new FakeFont(coverage, greek, cyrillic)` |
| `add_devanagari()`         | `addDevanagari()`                      |
| `add_standard_arabic()`    | `addStandardArabic()`                  |
| `add_urdu_and_farsi()`     | `addUrduAndFarsi()`                    |
| `add_bengali()`            | `addBengali()`                         |
| `add_thai()`               | `addThai()`                            |
| `add_weight_axis(low, high)` | `addWeightAxis(low, high)`           |
| `add_width_axis(low, high)` | `addWidthAxis(low, high)`             |
| `add_arbitrary_axis(tag, name, low, high, default, metrics, kerning)` | `addArbitraryAxis(…)` |
| `fill_out_masters(k, aw)`  | `fillOutMasters(k, aw)`                |
| `compile()`                | `compile()` → `Uint8Array`             |
| `table_stats(&[u8])`       | `tableStats(Uint8Array)` → `Map`       |

`latinCoverage` is `"full"`, `"core"` or `"kernel"`; anything else throws.

`addArbitraryAxis` validates before calling the library, which would otherwise
panic (and take the wasm module down with it) on a tag that isn't exactly four
printable ASCII bytes. It also rejects non-finite coordinates, `low >= high`,
and a `default` outside `low..high`. Note the library's argument order:
`low`, `high`, `default`.

## Building

From this directory:

```sh
wasm-pack build --target web --out-dir ../web/pkg
```

or from the repository root:

```sh
wasm-pack build fakefont-web --target web --out-dir ../web/pkg
```

`--target web` is what the page needs: it emits an ES module with a default
`init()` export and no bundler requirement. The output is git-ignored, so run
this after cloning (and after changing the library).

## Notes

- Adding Greek and Cyrillic is a constructor flag rather than a call, because
  `fakefont` slices them out of the Latin font in one pass; Devanagari, Arabic,
  Bengali and Thai come from separate fonts and are merged in afterwards.
- `addUrduAndFarsi` merges the whole Naskh Arabic font, which already contains
  the standard Arabic kernel, so the two Arabic tiles compose safely — the
  second call only adds what is still missing.
- `rand` needs entropy on wasm, so `getrandom`'s `wasm_js` backend is enabled
  for wasm targets only (see `Cargo.toml`); no `.cargo/config.toml` is needed.
- `cargo test -p fakefont-web` runs on the host and exercises the full
  build/compile/measure sequence without a browser.

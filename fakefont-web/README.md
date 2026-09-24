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

// Latin coverage, then the extra scripts, in any order.
const font = new FakeFont("core", ["cyrillic", "devanagari", "thai"]);

font.addWeightAxis(100, 700); // omit for no wght axis
font.addWidthAxis(75, 125); // omit for no wdth axis
font.addArbitraryAxis("slnt", "Slant", -15, 15, 0, true, true); // tag, name, low, high, default, …
font.fillOutMasters(/* adjust kerning */ true, /* adjust advance widths */ true);

const bytes = font.compile(); // Uint8Array
const tables = tableStats(bytes); // Map<string, number>
```

| Rust                       | JavaScript                             |
| -------------------------- | -------------------------------------- |
| `FakeFont::new(coverage, &subsets)` | `new FakeFont(coverage, subsets)` |
| `add_weight_axis(low, high)` | `addWeightAxis(low, high)`           |
| `add_width_axis(low, high)` | `addWidthAxis(low, high)`             |
| `add_arbitrary_axis(tag, name, low, high, default, metrics, kerning)` | `addArbitraryAxis(…)` |
| `fill_out_masters(k, aw)`  | `fillOutMasters(k, aw)`                |
| `master_count()`           | `masterCount()`                        |
| `glyph_count()`            | `glyphCount()`                         |
| `compile()`                | `compile()` → `Uint8Array`             |
| `table_stats(&[u8])`       | `tableStats(Uint8Array)` → `Map`       |

`latinCoverage` is `"full"`, `"core"` or `"kernel"`; anything else throws.

`subsets` lists the extra scripts to include, in any order and safe to repeat:

```
greek  cyrillic  cjk-basic  devanagari  bengali  standard-arabic
farsi-urdu  thai  tamil  telugu  kannada
```

These are the `value` attributes of the script tiles in `web/index.html`, so the
page passes its checked boxes straight through. An unknown name throws.

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

- The script list goes to the constructor rather than being added call by call,
  because `fakefont` cuts the subset out of one source font in a single pass and
  the result is fixed once built. `parse_subset` in `src/lib.rs` is the only
  place a script is named, so adding one means an arm there, an entry in
  `OtherSubsets`, and a tile in `web/index.html`.
- `farsi-urdu` pulls in the whole Naskh Arabic glyphset, which already contains
  the standard Arabic kernel, so the two Arabic tiles compose safely.
- `compile()` consumes the font. The library moves its data into the compiler
  rather than cloning it — worth having for a multi-megabyte subset — and
  wasm-bindgen zeroes the JS object's pointer as part of that, so any later call
  on it throws "null pointer passed to rust". Read `masterCount()` and
  `glyphCount()` before compiling.
- `rand` needs entropy on wasm, so `getrandom`'s `wasm_js` backend is enabled
  for wasm targets only (see `Cargo.toml`); no `.cargo/config.toml` is needed.
- `cargo test -p fakefont-web` runs on the host and exercises the full
  build/compile/measure sequence without a browser.

//! WebAssembly bindings for [`fakefont`].
//!
//! This crate is deliberately thin: it maps the library's Rust API onto the
//! JavaScript API that the "How Big Is A Font?" page in `web/` expects, and
//! turns the library's errors into real JavaScript `Error` objects.
//!
//! ```js
//! import init, { FakeFont, tableStats } from "./pkg/fakefont_web.js";
//! await init();
//!
//! // Latin coverage first, then the extra scripts, in any order.
//! const font = new FakeFont("core", ["cyrillic", "devanagari", "thai"]);
//! font.addAxis("wght", "Weight", 100, 700, 400, true, true);
//! font.addAxis("slnt", "Slant", -15, 15, 0, true, true);
//! font.fillOutMasters(/* adjust kerning */ true, /* adjust advance widths */ true);
//!
//! // Read these first: compile() consumes the font.
//! font.masterCount(); // masters created by fillOutMasters
//! font.glyphCount(); // glyphs in the font
//!
//! const bytes = font.compile();     // Uint8Array
//! const tables = tableStats(bytes); // Map<string, number>
//! ```

use wasm_bindgen::JsError;
use wasm_bindgen::prelude::*;

use js_sys::Map;

/// A synthetic font under construction.
#[wasm_bindgen]
pub struct FakeFont {
    inner: fakefont::FakeFont,
}

/// Plain Rust plumbing, kept out of the `#[wasm_bindgen]` block so that
/// `cargo test` can drive the whole pipeline on the host, where `JsError` and
/// the rest of the wasm ABI are not usable.
impl FakeFont {
    fn build(latin_coverage: &str, subsets: &[String]) -> Result<Self, String> {
        let mut parsed = Vec::with_capacity(subsets.len());
        for name in subsets {
            parsed.push(fakefont::OtherSubsets::from_name(name)?);
        }
        // The names are the page's tile values, and the library owns the list
        // of them: the key format writes the same ones.
        let inner =
            fakefont::FakeFont::new(fakefont::LatinCoverage::from_name(latin_coverage)?, &parsed)
                .map_err(|error| error.to_string())?;
        Ok(FakeFont { inner })
    }

    fn compile_bytes(self) -> Result<Vec<u8>, String> {
        self.inner.compile().map_err(|error| error.to_string())
    }

    /// Validates an axis and hands it to the library.
    ///
    /// The checks live in the library, next to `add_axis`, because the key
    /// format needs them too.
    // The argument list mirrors `fakefont::FakeFont::add_axis`, which is the
    // point of this crate.
    #[allow(clippy::too_many_arguments)]
    fn add_axis_checked(
        &mut self,
        tag: &str,
        name: &str,
        low: f64,
        high: f64,
        default: f64,
        affects_metrics: bool,
        affects_kerning: bool,
    ) -> Result<(), String> {
        fakefont::check_axis(tag, low, high, default)?;
        self.inner.add_axis(
            tag,
            name,
            low,
            high,
            default,
            affects_metrics,
            affects_kerning,
        );
        Ok(())
    }
}

#[wasm_bindgen]
impl FakeFont {
    /// `latinCoverage` is `"full"`, `"core"` or `"kernel"`; `subsets` names the
    /// extra scripts to include, in any order and with duplicates allowed.
    ///
    /// The names are the values of the script tiles on the page: `"greek"`,
    /// `"cyrillic"`, `"devanagari"`, `"standard-arabic"`, `"farsi-urdu"`,
    /// `"cjk-basic"`, `"bengali"`, `"thai"`, `"tamil"`, `"telugu"`,
    /// `"kannada"`, `"malayalam"`, `"gujarati"`, `"gurmukhi"`, `"oriya"`,
    /// `"khmer"`, `"lao"`, `"myanmar"`, `"ethiopic"`, `"armenian"` and
    /// `"georgian"`. Anything else throws.
    ///
    /// Every subset is chosen here rather than one call per script, because the
    /// font is cut out of the source data in a single pass and cannot gain
    /// scripts afterwards.
    #[wasm_bindgen(constructor)]
    pub fn new(latin_coverage: &str, subsets: Vec<String>) -> Result<FakeFont, JsError> {
        Self::build(latin_coverage, &subsets).map_err(|error| JsError::new(&error))
    }

    /// Add an axis, whose `tag` is four printable ASCII bytes.
    ///
    /// Note the argument order: `low`, `high`, then `default`, matching the
    /// library. `affectsMetrics` and `affectsKerning` say whether the axis
    /// varies advance widths and kerning; false freezes them across that axis.
    /// A `wght` axis also gets the library's weight warping map.
    ///
    /// Invalid input is rejected here rather than in the library, which pads
    /// the tag and unwraps it — a panic would abort the whole wasm module.
    // One argument per part of an axis, in the library's order.
    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = addAxis)]
    pub fn add_axis(
        &mut self,
        tag: &str,
        name: &str,
        low: f64,
        high: f64,
        default: f64,
        affects_metrics: bool,
        affects_kerning: bool,
    ) -> Result<(), JsError> {
        self.add_axis_checked(
            tag,
            name,
            low,
            high,
            default,
            affects_metrics,
            affects_kerning,
        )
        .map_err(|error| JsError::new(&error))
    }

    /// Create a master for every corner of the designspace built so far.
    #[wasm_bindgen(js_name = fillOutMasters)]
    pub fn fill_out_masters(&mut self, kerning: bool, advance_widths: bool) {
        self.inner.fill_out_masters(kerning, advance_widths);
    }

    /// How many masters [`FakeFont::fill_out_masters`] created.
    #[wasm_bindgen(js_name = masterCount)]
    pub fn master_count(&self) -> usize {
        self.inner.master_count()
    }

    /// How many glyphs the font holds.
    #[wasm_bindgen(js_name = glyphCount)]
    pub fn glyph_count(&self) -> usize {
        self.inner.glyph_count()
    }

    /// Compile to an OpenType binary, returned as a `Uint8Array`.
    ///
    /// This **consumes the font**. Taking `self` lets the library hand its data
    /// to the compiler instead of cloning it, which is worth having for the
    /// multi-megabyte subsets the page can ask for. wasm-bindgen zeroes the
    /// JavaScript object's pointer as part of that, so any later call on it
    /// fails with "null pointer passed to rust" — read
    /// [`FakeFont::master_count`] and [`FakeFont::glyph_count`] first.
    pub fn compile(self) -> Result<Vec<u8>, JsError> {
        self.compile_bytes().map_err(|error| JsError::new(&error))
    }
}

/// The byte length of every table in a compiled font, as a `Map<string, number>`.
///
/// `HashMap` is not a wasm-bindgen type, so the library's map is copied into a
/// JavaScript `Map` here.
#[wasm_bindgen(js_name = tableStats)]
pub fn table_stats(font: &[u8]) -> Result<Map, JsError> {
    let stats = fakefont::table_stats(font).map_err(|error| JsError::new(&error))?;
    let map = Map::new();
    for (tag, length) in stats {
        let _ = map.set(&JsValue::from_str(&tag), &JsValue::from_f64(length as f64));
    }
    Ok(map)
}

/// Install a panic hook so that a Rust panic in the browser shows up as a
/// readable console message instead of an opaque `unreachable executed` trap.
#[wasm_bindgen(start)]
fn start() {
    console_error_panic_hook::set_once();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The script names carried by the tiles in `web/index.html`.
    const SCRIPTS: [&str; 21] = [
        "greek",
        "cyrillic",
        "devanagari",
        "standard-arabic",
        "farsi-urdu",
        "cjk-basic",
        "bengali",
        "thai",
        "tamil",
        "telugu",
        "kannada",
        "malayalam",
        "gujarati",
        "gurmukhi",
        "oriya",
        "khmer",
        "lao",
        "myanmar",
        "ethiopic",
        "armenian",
        "georgian",
    ];

    /// `build` takes owned names, because that is what wasm-bindgen hands us
    /// from a JavaScript array.
    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|name| (*name).to_string()).collect()
    }

    /// Drives the same call sequence the web app uses, on the host.
    #[test]
    fn builds_compiles_and_measures() {
        let mut font = FakeFont::build(
            "kernel",
            &names(&[
                "devanagari",
                "standard-arabic",
                "farsi-urdu",
                "bengali",
                "thai",
            ]),
        )
        .expect("build");
        font.add_axis_checked("wght", "Weight", 100.0, 700.0, 400.0, true, true)
            .unwrap();
        font.add_axis_checked("wdth", "Width", 75.0, 125.0, 100.0, true, true)
            .unwrap();
        font.fill_out_masters(true, true);
        let bytes = font.compile_bytes().expect("compile");

        assert!(bytes.len() > 10_000);
        let tables = fakefont::table_stats(&bytes).expect("table stats");
        assert!(tables.contains_key("glyf"), "tables: {tables:?}");
        assert!(tables.contains_key("gvar"), "tables: {tables:?}");
        assert!(tables.contains_key("fvar"), "tables: {tables:?}");
    }

    /// Every script the page offers should contribute glyphs, so a library
    /// rename that quietly dropped one shows up here.
    #[test]
    fn every_script_adds_glyphs() {
        let baseline = FakeFont::build("kernel", &[]).expect("build").glyph_count();
        assert!(baseline > 0);

        for name in SCRIPTS {
            let font = FakeFont::build("kernel", &names(&[name])).expect("build");
            assert!(
                font.glyph_count() > baseline,
                "{name} added no glyphs to {baseline}"
            );
        }
    }

    #[test]
    fn a_static_font_has_no_variation_tables() {
        let mut font = FakeFont::build("kernel", &[]).expect("build");
        font.fill_out_masters(false, false);
        let bytes = font.compile_bytes().expect("compile");
        let tables = fakefont::table_stats(&bytes).expect("table stats");
        assert!(!tables.contains_key("gvar"), "tables: {tables:?}");
        assert!(!tables.contains_key("fvar"), "tables: {tables:?}");
    }

    /// Bad axes have to be caught before the library sees them: `add_axis`
    /// panics on a tag it cannot use, and a panic aborts the wasm module.
    #[test]
    fn rejects_axes_the_library_would_panic_on() {
        let mut font = FakeFont::build("kernel", &[]).expect("build");

        // Too short / too long / not printable ASCII / not ASCII at all.
        for tag in ["abc", "abcde", "ab\tc", "ab\u{e7}d", ""] {
            assert!(
                font.add_axis_checked(tag, "Small", 0.0, 10.0, 5.0, true, true)
                    .is_err(),
                "{tag:?} should be rejected"
            );
        }
        // Numeric problems.
        for (low, high, default) in [
            (15.0, -15.0, 0.0),
            (0.0, 0.0, 0.0),
            (0.0, 10.0, 11.0),
            (0.0, 10.0, f64::NAN),
            (f64::NEG_INFINITY, 10.0, 0.0),
        ] {
            assert!(
                font.add_axis_checked("slnt", "Slant", low, high, default, true, true)
                    .is_err(),
                "{low}/{high}/{default} should be rejected"
            );
        }

        // A good one still goes through.
        assert!(
            font.add_axis_checked("slnt", "Slant", -15.0, 15.0, 0.0, true, true)
                .is_ok()
        );
        assert_eq!(font.master_count(), 1, "nothing was filled out yet");
    }

    /// The path the page takes when the user fills in a custom axis.
    #[test]
    fn builds_with_an_arbitrary_axis() {
        for (tag, name, low, high, default) in [
            // Default at zero, and the harder case where it is not.
            ("slnt", "Slant", -15.0, 15.0, 0.0),
            ("opsz", "Optical size", 6.0, 144.0, 12.0),
            ("GRAD", "Grade", -200.0, 150.0, 88.0),
        ] {
            let mut font = FakeFont::build("kernel", &[]).expect("build");
            font.add_axis_checked(tag, name, low, high, default, true, true)
                .expect("add axis");
            font.fill_out_masters(true, true);
            let bytes = font
                .compile_bytes()
                .unwrap_or_else(|error| panic!("compile {tag}: {error}"));

            let tables = fakefont::table_stats(&bytes).expect("table stats");
            assert!(tables.contains_key("fvar"), "{tag} tables: {tables:?}");
            assert!(tables.contains_key("gvar"), "{tag} tables: {tables:?}");
        }
    }

    /// The counts the page shows above the file size.
    #[test]
    fn reports_masters_and_glyphs() {
        let font = FakeFont::build("kernel", &[]).expect("build");
        assert_eq!(font.master_count(), 1, "one master before filling out");
        let latin_glyphs = font.glyph_count();
        assert!(latin_glyphs > 0);

        let with_devanagari = FakeFont::build("kernel", &names(&["devanagari"])).expect("build");
        assert!(
            with_devanagari.glyph_count() > latin_glyphs,
            "adding a script should add glyphs"
        );

        let mut font = FakeFont::build("kernel", &[]).expect("build");
        font.add_axis_checked("slnt", "Slant", -15.0, 15.0, 0.0, true, true)
            .expect("add axis");
        font.fill_out_masters(false, false);
        assert_eq!(font.master_count(), 3, "low, default and high corners");
    }

    /// The South Asian scripts whose feature code refers to unencoded variants
    /// of Latin glyphs. The merge used to drop those variants, so these three
    /// compiled to "Glyph hyphen.<script> not found".
    #[test]
    fn scripts_with_variants_compile() {
        for name in ["tamil", "telugu", "kannada"] {
            let mut font = FakeFont::build("kernel", &names(&[name])).expect("build");
            font.fill_out_masters(false, false);
            let bytes = font
                .compile_bytes()
                .unwrap_or_else(|error| panic!("{name} failed to compile: {error}"));
            assert!(bytes.len() > 5_000, "{name} produced {} bytes", bytes.len());
        }
    }

    #[test]
    fn arbitrary_axes_add_masters() {
        let mut one = FakeFont::build("kernel", &[]).expect("build");
        one.add_axis_checked("slnt", "Slant", -15.0, 15.0, 0.0, true, true)
            .expect("add axis");
        one.fill_out_masters(false, false);

        let mut two = FakeFont::build("kernel", &[]).expect("build");
        two.add_axis_checked("slnt", "Slant", -15.0, 15.0, 0.0, true, true)
            .expect("add axis");
        two.add_axis_checked("opsz", "Optical size", 6.0, 144.0, 12.0, true, true)
            .expect("add axis");
        two.fill_out_masters(false, false);

        // Each extra axis should add more corner masters, so a bigger font.
        let one_bytes = one.compile_bytes().expect("compile");
        let two_bytes = two.compile_bytes().expect("compile");
        assert!(
            two_bytes.len() > one_bytes.len(),
            "one axis: {}, two axes: {}",
            one_bytes.len(),
            two_bytes.len()
        );
    }
}

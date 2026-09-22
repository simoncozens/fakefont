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
//! const font = new FakeFont("core", /* greek */ false, /* cyrillic */ true);
//! font.addDevanagari();
//! font.addStandardArabic(); // or addUrduAndFarsi() for the whole Naskh font
//! font.addBengali();
//! font.addThai();
//! font.addWeightAxis(100, 700);
//! font.addArbitraryAxis("slnt", "Slant", -15, 15, 0, true, true);
//! font.fillOutMasters(/* adjust kerning */ true, /* adjust advance widths */ true);
//!
//! const bytes = font.compile();     // Uint8Array
//! const tables = tableStats(bytes); // Map<string, number>
//!
//! font.masterCount(); // masters created by fillOutMasters
//! font.glyphCount(); // glyphs in the font
//! ```

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsError;

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
    fn build(latin_coverage: &str, greek: bool, cyrillic: bool) -> Result<Self, String> {
        Ok(FakeFont {
            inner: fakefont::FakeFont::new(parse_latin_coverage(latin_coverage)?, greek, cyrillic),
        })
    }

    fn compile_bytes(&self) -> Result<Vec<u8>, String> {
        self.inner.compile().map_err(|error| error.to_string())
    }

    /// Rejects the axes `check_axis` refuses to let through.
    // The argument list mirrors `fakefont::FakeFont::add_arbitrary_axis`, which
    // is the point of this crate.
    #[allow(clippy::too_many_arguments)]
    fn add_axis(
        &mut self,
        tag: &str,
        name: &str,
        low: f64,
        high: f64,
        default: f64,
        affects_metrics: bool,
        affects_kerning: bool,
    ) -> Result<(), String> {
        check_axis(tag, low, high, default)?;
        self.inner.add_arbitrary_axis(
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
    /// `latinCoverage` is `"full"`, `"core"` or `"kernel"`; `greek` and
    /// `cyrillic` pull in those core glyphsets.
    ///
    /// Devanagari and Arabic are not flags here: they are merged in afterwards
    /// with [`FakeFont::add_devanagari`] and [`FakeFont::add_arabic`], so that
    /// the scripts sliced out of the Latin font can supply glyphs those fonts
    /// also have without colliding.
    #[wasm_bindgen(constructor)]
    pub fn new(latin_coverage: &str, greek: bool, cyrillic: bool) -> Result<FakeFont, JsError> {
        Self::build(latin_coverage, greek, cyrillic).map_err(|error| JsError::new(&error))
    }

    /// Merge the Devanagari glyphs, kerning and feature code into the font.
    #[wasm_bindgen(js_name = addDevanagari)]
    pub fn add_devanagari(&mut self) {
        self.inner.add_devanagari();
    }

    /// Merge the standard Arabic kernel subset of Naskh Arabic.
    #[wasm_bindgen(js_name = addStandardArabic)]
    pub fn add_standard_arabic(&mut self) {
        self.inner.add_standard_arabic();
    }

    /// Merge the whole Naskh Arabic font, Farsi and Urdu additions included.
    ///
    /// A superset of [`FakeFont::add_standard_arabic`], so asking for both is
    /// harmless — the second call only adds what is still missing.
    #[wasm_bindgen(js_name = addUrduAndFarsi)]
    pub fn add_urdu_and_farsi(&mut self) {
        self.inner.add_urdu_and_farsi();
    }

    /// Merge the Bengali glyphs, kerning and feature code into the font.
    #[wasm_bindgen(js_name = addBengali)]
    pub fn add_bengali(&mut self) {
        self.inner.add_bengali();
    }

    /// Merge the Thai glyphs, kerning and feature code into the font.
    #[wasm_bindgen(js_name = addThai)]
    pub fn add_thai(&mut self) {
        self.inner.add_thai();
    }

    /// Add a `wght` axis spanning `low`..`high` user units.
    #[wasm_bindgen(js_name = addWeightAxis)]
    pub fn add_weight_axis(&mut self, low: f64, high: f64) {
        self.inner.add_weight_axis(low, high);
    }

    /// Add a `wdth` axis spanning `low`..`high` user units.
    #[wasm_bindgen(js_name = addWidthAxis)]
    pub fn add_width_axis(&mut self, low: f64, high: f64) {
        self.inner.add_width_axis(low, high);
    }

    /// Add an axis with an arbitrary four-byte tag.
    ///
    /// Note the argument order: `low`, `high`, then `default`, matching the
    /// library. `affectsMetrics` and `affectsKerning` say whether the axis
    /// varies advance widths and kerning; false freezes them across that axis.
    ///
    /// Invalid input is rejected here rather than in the library, which pads
    /// the tag and unwraps it — a panic would abort the whole wasm module.
    // One argument per part of an axis, in the library's order.
    #[allow(clippy::too_many_arguments)]
    #[wasm_bindgen(js_name = addArbitraryAxis)]
    pub fn add_arbitrary_axis(
        &mut self,
        tag: &str,
        name: &str,
        low: f64,
        high: f64,
        default: f64,
        affects_metrics: bool,
        affects_kerning: bool,
    ) -> Result<(), JsError> {
        self.add_axis(
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
    pub fn compile(&self) -> Result<Vec<u8>, JsError> {
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

/// Map the web app's `latinCoverage` string onto the library's enum.
fn parse_latin_coverage(name: &str) -> Result<fakefont::LatinCoverage, String> {
    match name {
        "full" => Ok(fakefont::LatinCoverage::Full),
        "core" => Ok(fakefont::LatinCoverage::Core),
        "kernel" => Ok(fakefont::LatinCoverage::Kernel),
        other => Err(format!(
            "unknown Latin coverage {other:?}: expected \"full\", \"core\" or \"kernel\""
        )),
    }
}

/// Reject anything the library cannot turn into an axis.
///
/// The tag matters most: `fakefont` pads it to four bytes and calls
/// `Tag::new_checked(..).unwrap()`, so a tag that is too long or is not
/// printable ASCII panics. A panic in wasm aborts the module and poisons the
/// page, so it has to be caught before the call.
fn check_axis(tag: &str, low: f64, high: f64, default: f64) -> Result<(), String> {
    let bytes = tag.as_bytes();
    if bytes.len() != 4 || !bytes.iter().all(|byte| (0x20..=0x7e).contains(byte)) {
        return Err(format!(
            "axis tag {tag:?} must be exactly four printable ASCII characters"
        ));
    }
    if ![low, high, default].iter().all(|value| value.is_finite()) {
        return Err(format!(
            "axis {tag:?} has a coordinate that is not a finite number"
        ));
    }
    if low >= high {
        return Err(format!("axis {tag:?} needs low to be less than high"));
    }
    if default < low || default > high {
        return Err(format!(
            "axis {tag:?} needs its default to lie between low and high"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_coverage_names() {
        assert!(matches!(
            parse_latin_coverage("full"),
            Ok(fakefont::LatinCoverage::Full)
        ));
        assert!(matches!(
            parse_latin_coverage("core"),
            Ok(fakefont::LatinCoverage::Core)
        ));
        assert!(matches!(
            parse_latin_coverage("kernel"),
            Ok(fakefont::LatinCoverage::Kernel)
        ));
        assert!(parse_latin_coverage("Core").is_err());
        assert!(parse_latin_coverage("").is_err());
    }

    /// Drives the same call sequence the web app uses, on the host.
    #[test]
    fn builds_compiles_and_measures() {
        let mut font = FakeFont::build("kernel", true, false).expect("build");
        font.add_devanagari();
        font.add_standard_arabic();
        font.add_urdu_and_farsi();
        font.add_bengali();
        font.add_thai();
        font.add_weight_axis(100.0, 700.0);
        font.add_width_axis(75.0, 125.0);
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
        let empty = FakeFont::build("kernel", false, false).expect("build");
        let baseline = empty.glyph_count();

        #[allow(clippy::type_complexity)]
        let scripts: [(&str, fn(&mut FakeFont)); 5] = [
            ("devanagari", FakeFont::add_devanagari),
            ("standard arabic", FakeFont::add_standard_arabic),
            ("farsi and urdu", FakeFont::add_urdu_and_farsi),
            ("bengali", FakeFont::add_bengali),
            ("thai", FakeFont::add_thai),
        ];

        for (name, add) in scripts {
            let mut font = FakeFont::build("kernel", false, false).expect("build");
            add(&mut font);
            assert!(
                font.glyph_count() > baseline,
                "{name} added no glyphs to {baseline}"
            );
        }
    }

    #[test]
    fn a_static_font_has_no_variation_tables() {
        let mut font = FakeFont::build("kernel", false, false).expect("build");
        font.fill_out_masters(false, false);
        let bytes = font.compile_bytes().expect("compile");
        let tables = fakefont::table_stats(&bytes).expect("table stats");
        assert!(!tables.contains_key("gvar"), "tables: {tables:?}");
        assert!(!tables.contains_key("fvar"), "tables: {tables:?}");
    }

    #[test]
    fn accepts_well_formed_axes() {
        assert!(check_axis("slnt", -15.0, 15.0, 0.0).is_ok());
        assert!(check_axis("opsz", 6.0, 144.0, 12.0).is_ok());
        // A default sitting on either end of the range is fine.
        assert!(check_axis("XXXX", 0.0, 100.0, 0.0).is_ok());
        assert!(check_axis("XXXX", 0.0, 100.0, 100.0).is_ok());
    }

    #[test]
    fn rejects_axes_the_library_would_panic_on() {
        // Too short / too long / not printable ASCII / not ASCII at all.
        assert!(check_axis("abc", 0.0, 10.0, 5.0).is_err());
        assert!(check_axis("abcde", 0.0, 10.0, 5.0).is_err());
        assert!(check_axis("ab\tc", 0.0, 10.0, 5.0).is_err());
        assert!(check_axis("abçd", 0.0, 10.0, 5.0).is_err());
        assert!(check_axis("", 0.0, 10.0, 5.0).is_err());
        // Numeric problems.
        assert!(check_axis("slnt", 15.0, -15.0, 0.0).is_err());
        assert!(check_axis("slnt", 0.0, 0.0, 0.0).is_err());
        assert!(check_axis("slnt", 0.0, 10.0, 11.0).is_err());
        assert!(check_axis("slnt", 0.0, 10.0, f64::NAN).is_err());
        assert!(check_axis("slnt", f64::NEG_INFINITY, 10.0, 0.0).is_err());
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
            let mut font = FakeFont::build("kernel", false, false).expect("build");
            font.add_axis(tag, name, low, high, default, true, true)
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
        let mut font = FakeFont::build("kernel", false, false).expect("build");
        assert_eq!(font.master_count(), 1, "one master before filling out");
        let latin_glyphs = font.glyph_count();
        assert!(latin_glyphs > 0);

        font.add_devanagari();
        assert!(
            font.glyph_count() > latin_glyphs,
            "merging a script should add glyphs"
        );

        font.add_axis("slnt", "Slant", -15.0, 15.0, 0.0, true, true)
            .expect("add axis");
        font.fill_out_masters(false, false);
        assert_eq!(font.master_count(), 3, "low, default and high corners");
    }

    #[test]
    fn arbitrary_axes_add_masters() {
        let mut one = FakeFont::build("kernel", false, false).expect("build");
        one.add_axis("slnt", "Slant", -15.0, 15.0, 0.0, true, true)
            .expect("add axis");
        one.fill_out_masters(false, false);

        let mut two = FakeFont::build("kernel", false, false).expect("build");
        two.add_axis("slnt", "Slant", -15.0, 15.0, 0.0, true, true)
            .expect("add axis");
        two.add_axis("opsz", "Optical size", 6.0, 144.0, 12.0, true, true)
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

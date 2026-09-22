//! A fake font library for testing and development purposes.
//!
//! Provides a `FakeFont` type that can be used to generate fonts with specific scripts and axes for testing purposes.
#![deny(missing_docs)]
use babelfont::filters::{FontFilter, RetainGlyphs};
use babelfont::{Axis, FormatSpecific};
use babelfont::{
    BabelfontError, DesignCoord, DesignLocation, Font, LayerType::DefaultForMaster, SmolStr, Tag,
    UserCoord,
};
use flate2::read::GzDecoder;
use fontmerge::{GlyphsetFilter, fontsubset};
use google_fonts_glyphsets::{
    GF_ARABIC_KERNEL, GF_CYRILLIC_CORE, GF_GREEK_CORE, GF_LATIN_AFRICAN, GF_LATIN_CORE,
    GF_LATIN_KERNEL, GF_LATIN_PLUS, GF_LATIN_VIETNAMESE,
};
use indexmap::IndexMap;
use rand::RngExt as _;
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::sync::LazyLock;

fn unzip_and_babelfont(bytes: &[u8]) -> Font {
    let mut d = GzDecoder::new(bytes);
    let mut s = String::new();
    d.read_to_string(&mut s).unwrap();
    serde_json::from_str(&s).unwrap()
}

static NOTO_SANS: LazyLock<Font> =
    LazyLock::new(|| unzip_and_babelfont(include_bytes!("../resources/notosans.babelfont.gz")));

static NOTO_SANS_DEVANAGARI: LazyLock<Font> = LazyLock::new(|| {
    unzip_and_babelfont(include_bytes!(
        "../resources/notosansdevanagari.babelfont.gz"
    ))
});

static NOTO_NASKH_ARABIC: LazyLock<Font> = LazyLock::new(|| {
    unzip_and_babelfont(include_bytes!("../resources/notonaskharabic.babelfont.gz"))
});

static NOTO_SANS_BENGALI: LazyLock<Font> = LazyLock::new(|| {
    unzip_and_babelfont(include_bytes!("../resources/notosansbengali.babelfont.gz"))
});
static NOTO_SANS_THAI: LazyLock<Font> =
    LazyLock::new(|| unzip_and_babelfont(include_bytes!("../resources/notosansthai.babelfont.gz")));

static NOTO_SANS_CJK_BASIC: LazyLock<Font> =
    LazyLock::new(|| unzip_and_babelfont(include_bytes!("../resources/CJK-8k.babelfont.gz")));
static NOTO_SANS_KANNADA: LazyLock<Font> = LazyLock::new(|| {
    unzip_and_babelfont(include_bytes!("../resources/notosanskannada.babelfont.gz"))
});
static NOTO_SANS_TAMIL: LazyLock<Font> = LazyLock::new(|| {
    unzip_and_babelfont(include_bytes!("../resources/notosanstamil.babelfont.gz"))
});
static NOTO_SANS_TELUGU: LazyLock<Font> = LazyLock::new(|| {
    unzip_and_babelfont(include_bytes!("../resources/notosanstelugu.babelfont.gz"))
});

/// Represents the coverage level of Latin characters in a fake font.
#[derive(PartialEq, Eq)]
pub enum LatinCoverage {
    /// Full Latin coverage, including core, kernel, African, Vietnamese, and additional Latin characters.
    Full,
    /// Google Fonts' expected core Latin coverage, including only the essential Latin characters and diacritic combinations.
    Core,
    /// Kernel Latin coverage, including the minimal set of Latin characters.
    Kernel,
}

/// The main type representing a fake font.
pub struct FakeFont(Font);

impl FakeFont {
    /// Creates a new fake font with the specified Latin coverage and optional Greek and Cyrillic scripts.
    pub fn new(latin_coverage: LatinCoverage, greek: bool, cyrillic: bool) -> Self {
        let mut font = FakeFont(NOTO_SANS.clone());
        let mut wanted_cps = HashSet::new();

        match latin_coverage {
            LatinCoverage::Core => {
                wanted_cps.extend(GF_LATIN_CORE.iter_codepoints().collect::<Vec<u32>>())
            }
            LatinCoverage::Kernel => {
                wanted_cps.extend(GF_LATIN_KERNEL.iter_codepoints().collect::<Vec<u32>>())
            }
            LatinCoverage::Full => {
                wanted_cps.extend(GF_LATIN_CORE.iter_codepoints().collect::<Vec<u32>>());
                wanted_cps.extend(GF_LATIN_KERNEL.iter_codepoints().collect::<Vec<u32>>());
                wanted_cps.extend(GF_LATIN_AFRICAN.iter_codepoints().collect::<Vec<u32>>());
                wanted_cps.extend(GF_LATIN_VIETNAMESE.iter_codepoints().collect::<Vec<u32>>());
                wanted_cps.extend(GF_LATIN_PLUS.iter_codepoints().collect::<Vec<u32>>());
            }
        };
        if greek {
            wanted_cps.extend(GF_GREEK_CORE.iter_codepoints().collect::<Vec<u32>>());
        }
        if cyrillic {
            wanted_cps.extend(GF_CYRILLIC_CORE.iter_codepoints().collect::<Vec<u32>>());
        }

        let codepoint_map = font
            .0
            .glyphs
            .iter()
            .flat_map(|g| g.codepoints.iter().map(|c| (c, g.name.clone())))
            .collect::<HashMap<&u32, SmolStr>>();
        let retained_glyphs = wanted_cps
            .into_iter()
            .flat_map(|g| codepoint_map.get(&g))
            .map(|gn| gn.to_string())
            .collect::<Vec<String>>();
        let _ = RetainGlyphs::new(retained_glyphs).apply(&mut font.0);
        font
    }

    /// Adds the Devanagari script to the fake font.
    pub fn add_devanagari(&mut self) {
        self.add_subfont(&NOTO_SANS_DEVANAGARI)
    }

    /// Adds the standard Arabic script to the fake font.
    pub fn add_standard_arabic(&mut self) {
        let mut dummy = Font::default();
        let include_codepoints = GF_ARABIC_KERNEL
            .iter_codepoints()
            .flat_map(char::from_u32)
            .collect::<Vec<char>>();
        let glyphset = GlyphsetFilter::new_from_codepoints(
            include_codepoints,
            &mut dummy,
            &NOTO_NASKH_ARABIC,
            fontmerge::ExistingGlyphHandling::Replace,
        );

        let subset = fontsubset(
            NOTO_NASKH_ARABIC.clone(),
            glyphset,
            fontmerge::LayoutHandling::Closure,
            false,
        )
        .unwrap();
        self.add_subfont(&subset)
    }

    /// Adds the Urdu and Farsi scripts to the fake font.
    pub fn add_urdu_and_farsi(&mut self) {
        self.add_subfont(&NOTO_NASKH_ARABIC)
    }
    /// Adds the Bengali script to the fake font.
    pub fn add_bengali(&mut self) {
        self.add_subfont(&NOTO_SANS_BENGALI)
    }
    /// Adds the CJK basic set to the fake font.
    pub fn add_cjk_basic(&mut self) {
        self.add_subfont(&NOTO_SANS_CJK_BASIC)
    }

    /// Adds the Thai script to the fake font.
    pub fn add_thai(&mut self) {
        self.add_subfont(&NOTO_SANS_THAI)
    }
    /// Adds the Tamil script to the fake font.
    pub fn add_tamil(&mut self) {
        self.add_subfont(&NOTO_SANS_TAMIL)
    }
    /// Adds the Telugu script to the fake font.
    pub fn add_telugu(&mut self) {
        self.add_subfont(&NOTO_SANS_TELUGU)
    }
    /// Adds the Kannada script to the fake font.
    pub fn add_kannada(&mut self) {
        self.add_subfont(&NOTO_SANS_KANNADA)
    }

    fn add_subfont(&mut self, subfont: &Font) {
        // Tracked as we go, so glyphs added by this call can't collide with
        // each other either.
        let mut taken_names = self
            .0
            .glyphs
            .iter()
            .map(|g| g.name.clone())
            .collect::<HashSet<SmolStr>>();
        let mut taken_codepoints = self
            .0
            .glyphs
            .iter()
            .flat_map(|g| g.codepoints.iter())
            .cloned()
            .collect::<HashSet<u32>>();
        for subfont_glyph in subfont.glyphs.iter() {
            if taken_names.contains(&subfont_glyph.name) {
                continue;
            }
            let mut glyph = subfont_glyph.clone();
            // A codepoint the font already maps belongs to whichever glyph got
            // there first, but that is no reason to drop this one. Script
            // variants like `hyphen.tamil` or `slash.UIknda` deliberately share
            // a codepoint with the Latin glyph and exist to be referenced by
            // that script's feature code; dropping them leaves the feature code
            // pointing at a glyph that isn't there, which fontir rejects with
            // "Glyph X not found". Keep the glyph, drop only the codepoint, so
            // the cmap stays unambiguous and the feature code still resolves.
            glyph.codepoints.retain(|c| !taken_codepoints.contains(c));
            for layer in glyph.layers.iter_mut() {
                layer.master = DefaultForMaster(self.0.masters[0].id.clone());
            }
            taken_names.insert(glyph.name.clone());
            taken_codepoints.extend(glyph.codepoints.iter().cloned());
            self.0.glyphs.push(glyph);
        }
        self.0.masters[0]
            .kerning
            .extend(subfont.masters[0].kerning.clone());
        self.0
            .features
            .classes
            .extend(subfont.features.classes.clone());
        self.0.features.prefixes.extend(
            subfont
                .features
                .prefixes
                .clone()
                .into_iter()
                .filter(|(_s, c)| !c.code.contains("languagesystem")),
        );
    }

    /// Adds a weight axis to the fake font with the specified low and high values.
    pub fn add_weight_axis(&mut self, low: f64, high: f64) {
        let mut map = vec![];
        let default = UserCoord::new(400.0);
        // Add in low->low, high->high, default->default plus some arbitrary warping
        map.push((UserCoord::new(low), DesignCoord::new(low)));
        map.push((UserCoord::new(high), DesignCoord::new(high)));
        map.push((default, DesignCoord::new(400.0)));
        if low < 400.0 {
            map.push((
                UserCoord::new((low + 400.0) / 2.0),
                DesignCoord::new((low + 400.0) / 2.0 + 50.0),
            ));
        }
        if high > 400.0 {
            map.push((
                UserCoord::new((high + 400.0) / 2.0),
                DesignCoord::new((high + 400.0) / 2.0 - 50.0),
            ));
        }
        map.sort();
        map.dedup();
        self.0.axes.push(babelfont::Axis {
            name: "Weight".into(),
            tag: Tag::new(b"wght"),
            min: Some(UserCoord::new(low)),
            max: Some(UserCoord::new(high)),
            default: Some(UserCoord::new(400.0)),
            map: Some(map),
            ..Default::default()
        })
    }

    /// Adds a width axis to the fake font with the specified low and high values.  
    pub fn add_width_axis(&mut self, low: f64, high: f64) {
        self.0.axes.push(babelfont::Axis {
            name: "Width".into(),
            tag: Tag::new(b"wdth"),
            min: Some(UserCoord::new(low)),
            max: Some(UserCoord::new(high)),
            default: Some(UserCoord::new(100.0)),
            ..Default::default()
        })
    }

    /// Adds an arbitrary axis to the fake font with the specified tag, name, low, high, default values, and flags indicating whether it affects metrics and kerning.
    #[allow(clippy::too_many_arguments)]
    pub fn add_arbitrary_axis(
        &mut self,
        tag: &str,
        name: &str,
        low: f64,
        high: f64,
        default: f64,
        affects_metrics: bool,
        affects_kerning: bool,
    ) {
        let fourbyte_tag = &format!("{: <4}", tag)[0..4];
        let mut fs = FormatSpecific::default();
        fs.insert(
            "affects_metrics".to_string(),
            serde_json::Value::Bool(affects_metrics),
        );
        fs.insert(
            "affects_kerning".to_string(),
            serde_json::Value::Bool(affects_kerning),
        );
        let mut map = vec![
            (UserCoord::new(low), DesignCoord::new(low)),
            (UserCoord::new(default), DesignCoord::new(default)),
            (UserCoord::new(high), DesignCoord::new(high)),
        ];
        map.dedup_by_key(|(user, _design)| user.to_f64());
        self.0.axes.push(babelfont::Axis {
            name: name.into(),
            tag: Tag::new_checked(fourbyte_tag.as_bytes()).unwrap(),
            min: Some(UserCoord::new(low)),
            max: Some(UserCoord::new(high)),
            default: Some(UserCoord::new(default)),
            map: Some(map),
            format_specific: fs,
            ..Default::default()
        })
    }

    /// Creates additional master locations for the fake font based on the specified axes, filling out the corners of the design space.
    ///
    /// This must be called after all axes have been added to the fake font and before the font is compiled.
    pub fn fill_out_masters(&mut self, kerning: bool, advance_widths: bool) {
        // Make first master the default
        let mut default_location = DesignLocation::default();
        for axis in &self.0.axes {
            default_location.insert(
                axis.tag,
                axis.default
                    .map(|uc| DesignCoord::new(uc.to_f64()))
                    .unwrap_or(DesignCoord::new(0.0)),
            );
        }
        self.0.masters[0].location = default_location;
        // Cartesian product top, default, and bottom of all axes to find corner master locations
        let mut corner_locations = vec![];
        for axis in &self.0.axes {
            let min = axis
                .min
                .map(|uc| DesignCoord::new(uc.to_f64()))
                .unwrap_or(DesignCoord::new(0.0));
            let max = axis
                .max
                .map(|uc| DesignCoord::new(uc.to_f64()))
                .unwrap_or(DesignCoord::new(0.0));
            let default = axis
                .default
                .map(|uc| DesignCoord::new(uc.to_f64()))
                .unwrap_or(DesignCoord::new(0.0));
            if corner_locations.is_empty() {
                corner_locations.push({
                    let mut loc = DesignLocation::default();
                    loc.insert(axis.tag, min);
                    loc
                });
                corner_locations.push({
                    let mut loc = DesignLocation::default();
                    loc.insert(axis.tag, max);
                    loc
                });
                corner_locations.push({
                    let mut loc = DesignLocation::default();
                    loc.insert(axis.tag, default);
                    loc
                });
            } else {
                let mut new_corners = vec![];
                for loc in &corner_locations {
                    let mut loc_min = loc.clone();
                    loc_min.insert(axis.tag, min);
                    new_corners.push(loc_min);
                    let mut loc_max = loc.clone();
                    loc_max.insert(axis.tag, max);
                    new_corners.push(loc_max);
                    let mut loc_default = loc.clone();
                    loc_default.insert(axis.tag, default);
                    new_corners.push(loc_default);
                }
                corner_locations = new_corners;
            }
        }
        for loc in corner_locations {
            // Skip if we already have one.
            if self.0.masters.iter().any(|m| m.location == loc) {
                continue;
            }
            let mut new_master = babelfont::Master {
                name: format!("{:?}", loc).into(),
                id: format!("{:?}", loc),
                location: loc,
                kerning: self.0.masters[0].kerning.clone(),
                metrics: self.0.masters[0].metrics.clone(),
                ..Default::default()
            };
            // Create layers, adjust glyph positions
            let axes = &self.0.axes;
            for glyph in self.0.glyphs.iter_mut() {
                if let Some(layer) = glyph.layers.first() {
                    let mut new_layer = layer.clone();
                    new_layer.master = DefaultForMaster(new_master.id.clone());
                    adjust_layer(&mut new_layer, axes, &new_master.location, advance_widths);
                    glyph.layers.push(new_layer);
                }
            }
            if kerning {
                adjust_kerning(&mut new_master.kerning, axes, &new_master.location);
            }
            self.0.masters.push(new_master);
        }
    }

    /// How many masters the designspace has been filled out with.
    pub fn master_count(&self) -> usize {
        self.0.masters.len()
    }

    /// How many glyphs the font currently holds.
    pub fn glyph_count(&self) -> usize {
        self.0.glyphs.len()
    }

    /// Compiles the fake font into a binary font file, returning the resulting bytes or an error.
    pub fn compile(&self) -> Result<Vec<u8>, BabelfontError> {
        use babelfont::convertors::fontir::CompilationOptions;

        let bytes = babelfont::convertors::fontir::BabelfontIrSource::compile(
            self.0.clone(),
            CompilationOptions::default(),
        )?;
        Ok(bytes)
    }
}

/// Returns a map of table tags to their lengths for the given font bytes.
pub fn table_stats(font: &[u8]) -> Result<HashMap<String, usize>, String> {
    let font_ref = skrifa::FontRef::new(font).map_err(|e| e.to_string())?;
    let mut stats = HashMap::new();
    for table in font_ref.table_directory().table_records() {
        stats.insert(table.tag().to_string(), table.length() as usize);
    }
    Ok(stats)
}

fn adjust_kerning(
    kerning: &mut IndexMap<(SmolStr, SmolStr), i16>,
    axes: &[Axis],
    loc: &DesignLocation,
) {
    // Drop axes which don't affect kerning
    let mut loc = loc.clone();
    loc.retain(|tag, _| {
        if let Some(axis) = axes.iter().find(|a| a.tag == *tag)
            && let Some(affects_kerning) = axis.format_specific.get("affects_kerning")
        {
            return affects_kerning.as_bool().unwrap_or(true);
        }
        true
    });
    kerning.iter_mut().for_each(|((_left, _right), value)| {
        *value = transform_x_coord(*value, &loc, *value) as i16;
    });
}

fn adjust_layer(
    layer: &mut babelfont::Layer,
    axes: &[Axis],
    loc: &DesignLocation,
    advance_widths: bool,
) {
    let Ok(bounds) = layer.bounds() else { return };
    let midpoint_x = bounds.center().x;
    let midpoint_y = bounds.center().y;

    for shape in layer.shapes.iter_mut() {
        match shape {
            babelfont::Shape::Component(component) => {
                component.transform.translation = (
                    transform_x_coord(component.transform.translation.0, loc, midpoint_x),
                    component.transform.translation.1,
                );
            }
            babelfont::Shape::Path(path) => {
                for point in path.nodes.iter_mut() {
                    point.x = transform_x_coord(point.x, loc, midpoint_x);
                    point.y = transform_y_coord(point.y, loc, midpoint_y);
                }
            }
        }
    }
    for anchor in layer.anchors.iter_mut() {
        anchor.x = transform_x_coord(anchor.x, loc, midpoint_x);
    }
    if advance_widths {
        let mut loc = loc.clone();
        // let axes = &font.axes;
        // Remove any axes which don't affect metrics
        loc.retain(|tag, _| {
            if let Some(axis) = axes.iter().find(|a| a.tag == *tag)
                && let Some(affects_metrics) = axis.format_specific.get("affects_metrics")
            {
                return affects_metrics.as_bool().unwrap_or(true);
            }
            true
        });
        layer.width = transform_x_coord(layer.width, &loc, midpoint_x) as f32;
    }
}

fn transform_x_coord(x: impl Into<f64>, loc: &DesignLocation, midpoint: impl Into<f64>) -> f64 {
    let mut rng = rand::rng();
    let mut x = x.into();
    if let Some(weight) = loc.get(Tag::new(b"wght")) {
        let factor = (weight.to_f64() - 400.0) / 800.0;
        // Push points toward (or away from) midpoint
        x += (x - midpoint.into()) * factor;
    }
    if let Some(width) = loc.get(Tag::new(b"wdth")) {
        x *= width.to_f64() / 100.0;
    }
    // Look for other axes
    for (tag, value) in loc.iter() {
        if tag != &Tag::new(b"wght") && tag != &Tag::new(b"wdth") {
            x += rng.random_range(value.to_f64() - 5.0..value.to_f64() + 5.0);
        }
    }
    // Jitter slightly to push up gvar
    x += rng.random_range(-2.0..5.0);

    x
}

fn transform_y_coord(y: impl Into<f64>, loc: &DesignLocation, midpoint: impl Into<f64>) -> f64 {
    let mut rng = rand::rng();
    let mut y = y.into();
    if let Some(weight) = loc.get(Tag::new(b"wght")) {
        let factor = (weight.to_f64() - 400.0) / 800.0;
        // Push points toward (or away from) midpoint
        y += (y - midpoint.into()) * factor;
    }
    for (tag, value) in loc.iter() {
        if tag != &Tag::new(b"wght") && tag != &Tag::new(b"wdth") {
            y += rng.random_range(value.to_f64() - 5.0..value.to_f64() + 5.0);
        }
    }
    y += rng.random_range(-2.0..5.0);
    y
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic() {
        let font_all = FakeFont::new(LatinCoverage::Full, false, false);
        assert_eq!(font_all.0.masters.len(), 1);
        assert_eq!(font_all.0.axes.len(), 0);
        assert_eq!(font_all.0.glyphs.iter().count(), 767);

        let font_core = FakeFont::new(LatinCoverage::Core, false, false);
        assert_eq!(font_core.0.masters.len(), 1);
        assert_eq!(font_core.0.glyphs.iter().count(), 319);

        let font_core = FakeFont::new(LatinCoverage::Core, true, false);
        assert_eq!(font_core.0.masters.len(), 1);
        assert_eq!(font_core.0.glyphs.iter().count(), 396);
    }

    #[test]
    fn test_masters() {
        let mut font_core = FakeFont::new(LatinCoverage::Full, false, false);
        font_core.add_weight_axis(100.0, 400.0);
        font_core.fill_out_masters(false, false);
        assert_eq!(font_core.0.axes.len(), 1);
        assert_eq!(font_core.0.masters.len(), 2);

        let mut font_core = FakeFont::new(LatinCoverage::Full, false, false);
        font_core.add_weight_axis(100.0, 700.0);
        font_core.fill_out_masters(false, false);
        assert_eq!(font_core.0.axes.len(), 1);
        assert_eq!(font_core.0.masters.len(), 3);

        // Add weight and width
        let mut font_core = FakeFont::new(LatinCoverage::Full, false, false);
        font_core.add_weight_axis(100.0, 700.0);
        font_core.add_width_axis(75.0, 100.0);
        font_core.fill_out_masters(false, false);
        assert_eq!(font_core.0.axes.len(), 2);
        assert_eq!(font_core.0.masters.len(), 6);

        let mut font_core = FakeFont::new(LatinCoverage::Full, false, false);
        font_core.add_weight_axis(100.0, 700.0);
        font_core.add_width_axis(75.0, 125.0);
        font_core.fill_out_masters(true, true);
        assert_eq!(font_core.0.axes.len(), 2);
        assert_eq!(font_core.0.masters.len(), 9);
    }

    #[test]
    fn test_add_deva_and_compile() {
        let mut font_core = FakeFont::new(LatinCoverage::Full, false, false);
        font_core.add_devanagari();
        font_core.add_weight_axis(100.0, 700.0);
        font_core.fill_out_masters(false, false);
        font_core.compile().expect("Compilation failed");

        // and arabic
        let mut font_core = FakeFont::new(LatinCoverage::Full, false, false);
        font_core.add_standard_arabic();
        font_core.add_weight_axis(100.0, 700.0);
        font_core.fill_out_masters(false, false);
        font_core.compile().expect("Compilation failed");

        // and both
        let mut font_core = FakeFont::new(LatinCoverage::Full, false, false);
        font_core.add_standard_arabic();
        font_core.add_devanagari();
        font_core.add_weight_axis(100.0, 700.0);
        font_core.fill_out_masters(false, false);
        font_core.compile().expect("Compilation failed");
    }

    #[test]
    fn test_arbitrary_axis() {
        let mut font_core = FakeFont::new(LatinCoverage::Full, false, false);
        font_core.add_arbitrary_axis("slnt", "Slaht", 0.0, 10.0, 0.0, true, true);
        font_core.fill_out_masters(false, false);
        assert_eq!(font_core.0.axes.len(), 1);
        assert_eq!(font_core.0.masters.len(), 2);
        println!("Map: {:?}", font_core.0.axes[0].map);
        font_core.compile().expect("Compilation failed");
    }

    /// Combining scripts must stay unambiguous, whatever the order they are
    /// merged in: no codepoint may end up on two glyphs (the cmap would be
    /// ambiguous) and no two glyphs may share a name (references would be). A
    /// script variant that shares a codepoint with a Latin glyph is kept, but
    /// loses the codepoint.
    #[test]
    fn merged_fonts_stay_unambiguous() {
        let mut font = FakeFont::new(LatinCoverage::Full, false, false);
        font.add_devanagari();
        font.add_tamil();
        font.add_telugu();
        font.add_kannada();
        font.add_standard_arabic();
        font.add_urdu_and_farsi();

        let mut by_codepoint: HashMap<u32, SmolStr> = HashMap::new();
        let mut by_name: HashSet<SmolStr> = HashSet::new();
        for glyph in font.0.glyphs.iter() {
            assert!(
                by_name.insert(glyph.name.clone()),
                "duplicate glyph name {}",
                glyph.name
            );
            for codepoint in glyph.codepoints.iter() {
                if let Some(other) = by_codepoint.insert(*codepoint, glyph.name.clone()) {
                    panic!("U+{codepoint:04X} is on both {other} and {}", glyph.name);
                }
            }
        }
    }

    #[test]
    fn combinations_compile() {
        let mut one = FakeFont::new(LatinCoverage::Full, false, false);
        one.add_tamil();
        one.fill_out_masters(false, false);
        let bytes = one.compile().expect("compile");
        assert!(!bytes.is_empty(), "compiled font should have non-zero size");

        let mut one = FakeFont::new(LatinCoverage::Full, false, false);
        one.add_devanagari();
        one.add_tamil();
        let bytes = one.compile().expect("compile");
        assert!(!bytes.is_empty(), "compiled font should have non-zero size");
    }
}

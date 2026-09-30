//! String keys for font options.
//!
//! A key spells out everything that goes into a [`FakeFont`]: the Latin
//! coverage, the extra scripts, the designspace axes and the two
//! master-interpolation switches. Keys exist so that a cache of compiled sizes
//! can be looked up by the options that produced them, and so that the cache can
//! be built natively — [`Options::to_key`] writes what a page (see `optionsKey`
//! in `web/app.js`) looks up, and [`FakeFont::from_key`] reads it back.
//!
//! A key is the version, then five `;`-separated fields:
//!
//! ```text
//! v1;<latin>;<scripts>;<axes>;<kerning>;<advance widths>
//! ```
//!
//! - `latin` is `full`, `core` or `kernel`.
//! - `scripts` is the page's script names joined with `+`, in the order they are
//!   merged. Empty means Latin only.
//! - `axes` is axis records joined with `,`, each
//!   `tag:name:low:default:high:metrics:kerning`, where the two flags are `1` or
//!   `0`. Empty means no axes.
//! - The last two fields are `1` or `0`.
//!
//! An axis name may be empty, and is then written as the axis tag: that is the
//! name the font ends up with, and the key has to describe the font that gets
//! built rather than the boxes that were filled in.
//!
//! Inside a tag or a name, `%` followed by two hex digits escapes the five
//! characters the grammar reserves (`%`, `;`, `:`, `,` and `+`), so a name like
//! `Slant, v2` survives a round trip. Numbers are written the way JavaScript
//! writes them — the shortest form that round-trips, never in exponent
//! notation — so that both sides agree on the string.
//!
//! ```text
//! v1;full;;wght:Weight:300:400:800:1:1,opsz:Optical Size:8:24:144:1:1;1;1
//! ```

use std::fmt;
use std::str::FromStr;

use crate::{FakeFont, LatinCoverage, OtherSubsets, check_axis};

/// The prefix every key starts with.
///
/// Bump it when the grammar changes, or when a change to the library would make
/// previously cached sizes wrong: it dates the cache as much as it versions the
/// format.
pub const KEY_VERSION: &str = "v1";

/// The characters the grammar reserves, escaped as `%XX` in tags and names.
const RESERVED: [char; 5] = ['%', ';', ':', ',', '+'];

/// The reason a key could not be turned into a font.
#[derive(Debug)]
pub enum KeyError {
    /// The key is not a string the format produces.
    Malformed(String),
    /// The key was understood, but the font it describes could not be built.
    Build(fontmerge::FontmergeError),
}

impl fmt::Display for KeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyError::Malformed(message) => write!(f, "malformed font key: {message}"),
            KeyError::Build(error) => write!(f, "the font the key describes: {error}"),
        }
    }
}

impl std::error::Error for KeyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            KeyError::Malformed(_) => None,
            KeyError::Build(error) => Some(error),
        }
    }
}

/// A key that did not parse, with an explanation of what is wrong with it.
fn malformed(message: impl Into<String>) -> KeyError {
    KeyError::Malformed(message.into())
}

/// One designspace axis, as the page's axis table describes it.
#[derive(Debug, Clone, PartialEq)]
pub struct AxisSpec {
    /// The four-byte axis tag, such as `wght` or `slnt`.
    pub tag: String,
    /// The axis name, which the font ends up carrying in `fvar`. Empty means
    /// "use the tag".
    pub name: String,
    /// The bottom of the user-coordinate range.
    pub low: f64,
    /// The user coordinate of the default master.
    pub default: f64,
    /// The top of the user-coordinate range.
    pub high: f64,
    /// Whether the axis varies advance widths.
    pub affects_metrics: bool,
    /// Whether the axis varies kerning.
    pub affects_kerning: bool,
}

impl AxisSpec {
    /// Rejects a tag or a range that [`FakeFont::add_axis`] would panic on.
    pub fn validate(&self) -> Result<(), String> {
        check_axis(&self.tag, self.low, self.high, self.default)
    }

    /// The name the font will use, which is the tag when none was given.
    fn font_name(&self) -> &str {
        if self.name.is_empty() {
            &self.tag
        } else {
            &self.name
        }
    }

    fn to_key(&self) -> String {
        [
            escape(&self.tag),
            escape(self.font_name()),
            coord(self.low),
            coord(self.default),
            coord(self.high),
            flag(self.affects_metrics).to_string(),
            flag(self.affects_kerning).to_string(),
        ]
        .join(":")
    }

    fn from_key(record: &str) -> Result<Self, KeyError> {
        let fields: Vec<&str> = record.split(':').collect();
        if fields.len() != 7 {
            return Err(malformed(format!(
                "an axis needs seven fields, tag:name:low:default:high:metrics:kerning, \
                 but this one has {}",
                fields.len()
            )));
        }
        let axis = AxisSpec {
            tag: unescape(fields[0])?,
            name: unescape(fields[1])?,
            low: coord_from_key(fields[2], "an axis low")?,
            default: coord_from_key(fields[3], "an axis default")?,
            high: coord_from_key(fields[4], "an axis high")?,
            affects_metrics: flag_from_key(fields[5], "affects metrics")?,
            affects_kerning: flag_from_key(fields[6], "affects kerning")?,
        };
        axis.validate().map_err(malformed)?;
        Ok(axis)
    }
}

/// Everything that goes into building a fake font.
///
/// This is the Rust side of the page's `Options` object, and what a key spells
/// out. Build one to enumerate the cases a size cache should hold, then hand
/// [`Options::to_key`] to [`FakeFont::from_key`].
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    /// How much of Latin to include.
    pub latin_coverage: LatinCoverage,
    /// The extra scripts, in the order they are merged.
    pub subsets: Vec<OtherSubsets>,
    /// The designspace axes, in the order they are applied.
    pub axes: Vec<AxisSpec>,
    /// Whether to interpolate kerning across the designspace.
    pub kerning: bool,
    /// Whether to interpolate advance widths across the designspace.
    pub advance_widths: bool,
}

impl Options {
    /// Spells the options out as the key that describes them.
    ///
    /// The string is the same one the page computes, so the two can be compared
    /// directly. An empty axis name is written as the axis tag.
    pub fn to_key(&self) -> String {
        let scripts = self
            .subsets
            .iter()
            .map(OtherSubsets::name)
            .collect::<Vec<_>>()
            .join("+");
        let axes = self
            .axes
            .iter()
            .map(AxisSpec::to_key)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{KEY_VERSION};{};{scripts};{axes};{};{}",
            self.latin_coverage.name(),
            flag(self.kerning),
            flag(self.advance_widths),
        )
    }

    /// Reads a key back into the options it was written from.
    ///
    /// Surrounding whitespace is ignored, so keys can be read from a file one
    /// per line.
    pub fn from_key(key: &str) -> Result<Self, KeyError> {
        let fields: Vec<&str> = key.trim().split(';').collect();
        if fields.len() != 6 {
            return Err(malformed(format!(
                "a key has six fields, version;latin;scripts;axes;kerning;advance widths, \
                 but this one has {}",
                fields.len()
            )));
        }
        if fields[0] != KEY_VERSION {
            return Err(malformed(format!(
                "unknown key version {:?}, expected {KEY_VERSION:?}",
                fields[0]
            )));
        }

        let latin_coverage = LatinCoverage::from_name(fields[1]).map_err(malformed)?;

        let subsets = if fields[2].is_empty() {
            Vec::new()
        } else {
            fields[2]
                .split('+')
                .map(|name| {
                    if name.is_empty() {
                        Err(malformed("a script field has an empty name in it"))
                    } else {
                        OtherSubsets::from_name(name).map_err(malformed)
                    }
                })
                .collect::<Result<Vec<_>, _>>()?
        };

        let axes = if fields[3].is_empty() {
            Vec::new()
        } else {
            fields[3]
                .split(',')
                .map(AxisSpec::from_key)
                .collect::<Result<Vec<_>, _>>()?
        };

        Ok(Options {
            latin_coverage,
            subsets,
            axes,
            kerning: flag_from_key(fields[4], "kerning")?,
            advance_widths: flag_from_key(fields[5], "advance widths")?,
        })
    }
}

impl FakeFont {
    /// Builds a font from a key, with its designspace filled out and ready to
    /// [`compile`](FakeFont::compile).
    ///
    /// This is [`Options::from_key`] followed by the same calls the page makes:
    /// one [`add_axis`](FakeFont::add_axis) per axis in the key, then
    /// [`fill_out_masters`](FakeFont::fill_out_masters) with the key's kerning
    /// and advance-width switches.
    pub fn from_key(key: &str) -> Result<Self, KeyError> {
        let options = Options::from_key(key)?;
        let mut font =
            FakeFont::new(options.latin_coverage, &options.subsets).map_err(KeyError::Build)?;
        for axis in &options.axes {
            font.add_axis(
                &axis.tag,
                axis.font_name(),
                axis.low,
                axis.high,
                axis.default,
                axis.affects_metrics,
                axis.affects_kerning,
            );
        }
        font.fill_out_masters(options.kerning, options.advance_widths);
        Ok(font)
    }
}

/// `1` or `0`, the way the grammar writes a boolean.
fn flag(value: bool) -> &'static str {
    if value { "1" } else { "0" }
}

fn flag_from_key(text: &str, what: &str) -> Result<bool, KeyError> {
    match text {
        "1" => Ok(true),
        "0" => Ok(false),
        other => Err(malformed(format!("{what} must be 1 or 0, not {other:?}"))),
    }
}

/// A coordinate in the spelling JavaScript's `String` uses for the same value.
///
/// Rust's `{}` and JavaScript agree on the shortest round-tripping decimal
/// everywhere but `-0`, which JavaScript writes as `0`.
fn coord(value: f64) -> String {
    let value = if value == 0.0 { 0.0 } else { value };
    format!("{value}")
}

fn coord_from_key(text: &str, what: &str) -> Result<f64, KeyError> {
    f64::from_str(text).map_err(|_| malformed(format!("{what} is not a number: {text:?}")))
}

/// Hides the characters the grammar gives a meaning to. Anything else, spaces
/// and non-ASCII included, is left alone so that keys stay readable.
fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        if RESERVED.contains(&character) {
            escaped.push_str(&format!("%{:02X}", character as u32));
        } else {
            escaped.push(character);
        }
    }
    escaped
}

/// The inverse of [`escape`], rejecting escapes that are not two hex digits
/// standing for a character.
fn unescape(text: &str) -> Result<String, KeyError> {
    let mut unescaped = String::with_capacity(text.len());
    let mut characters = text.chars();
    while let Some(character) = characters.next() {
        if character != '%' {
            unescaped.push(character);
            continue;
        }
        let digits: String = characters.by_ref().take(2).collect();
        if digits.len() != 2 {
            return Err(malformed(format!(
                "an escape is not two hex digits: %{digits}"
            )));
        }
        let code = u32::from_str_radix(&digits, 16)
            .map_err(|_| malformed(format!("an escape is not two hex digits: %{digits}")))?;
        let decoded = char::from_u32(code)
            .ok_or_else(|| malformed(format!("an escape is not a character: %{digits}")))?;
        unescaped.push(decoded);
    }
    Ok(unescaped)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The designspace the page starts with, as `DEFAULT_AXES` in `web/app.js`
    /// spells it.
    fn page_defaults() -> Options {
        Options {
            latin_coverage: LatinCoverage::Full,
            subsets: Vec::new(),
            axes: vec![
                AxisSpec {
                    tag: "wght".into(),
                    name: "Weight".into(),
                    low: 300.0,
                    default: 400.0,
                    high: 800.0,
                    affects_metrics: true,
                    affects_kerning: true,
                },
                AxisSpec {
                    tag: "wdth".into(),
                    name: "Width".into(),
                    low: 25.0,
                    default: 100.0,
                    high: 125.0,
                    affects_metrics: true,
                    affects_kerning: true,
                },
                AxisSpec {
                    tag: "opsz".into(),
                    name: "Optical Size".into(),
                    low: 8.0,
                    default: 24.0,
                    high: 144.0,
                    affects_metrics: true,
                    affects_kerning: true,
                },
                AxisSpec {
                    tag: "ROND".into(),
                    name: "Rounded".into(),
                    low: 0.0,
                    default: 0.0,
                    high: 100.0,
                    affects_metrics: false,
                    affects_kerning: false,
                },
            ],
            kerning: true,
            advance_widths: true,
        }
    }

    /// The page has to compute this exact string — `optionsKey(collectOptions())`
    /// on a fresh page — or the cache it looks up will never be found.
    #[test]
    fn writes_the_key_the_page_writes() {
        assert_eq!(
            page_defaults().to_key(),
            "v1;full;;wght:Weight:300:400:800:1:1,wdth:Width:25:100:125:1:1,\
             opsz:Optical Size:8:24:144:1:1,ROND:Rounded:0:0:100:0:0;1;1"
        );
    }

    #[test]
    fn reads_back_what_it_wrote() {
        let options = Options {
            latin_coverage: LatinCoverage::Kernel,
            subsets: vec![OtherSubsets::Greek, OtherSubsets::CjkBasic],
            axes: vec![
                AxisSpec {
                    tag: "slnt".into(),
                    name: "Slant".into(),
                    low: -15.5,
                    default: 0.0,
                    high: 15.0,
                    affects_metrics: true,
                    affects_kerning: false,
                },
                AxisSpec {
                    tag: "GRAD".into(),
                    name: String::new(),
                    low: -200.0,
                    default: 88.0,
                    high: 150.0,
                    affects_metrics: false,
                    affects_kerning: false,
                },
            ],
            kerning: false,
            advance_widths: true,
        };
        let key = options.to_key();
        let read = Options::from_key(&key).expect("the key should parse");

        // An empty name is written as the tag, so that is what comes back.
        let mut expected = options.clone();
        expected.axes[1].name = "GRAD".into();

        assert_eq!(read, expected);
        assert_eq!(read.to_key(), key, "the second key should be identical");
    }

    /// A tag or a name may contain the characters the grammar uses, so they have
    /// to be escaped rather than break the fields apart.
    #[test]
    fn escapes_the_reserved_characters() {
        let axis = AxisSpec {
            tag: "AB;D".into(),
            name: "Slant, v2: 100%+".into(),
            low: -1.0,
            default: -0.0,
            high: 1.0,
            affects_metrics: true,
            affects_kerning: true,
        };
        let key = Options {
            latin_coverage: LatinCoverage::Core,
            subsets: vec![OtherSubsets::Thai],
            axes: vec![axis.clone()],
            kerning: true,
            advance_widths: false,
        }
        .to_key();

        assert!(key.contains("AB%3BD:Slant%2C v2%3A 100%25%2B"), "{key}");
        assert_eq!(Options::from_key(&key).unwrap().axes, vec![axis]);
    }

    /// Negative zero is the one number the two languages spell differently.
    #[test]
    fn writes_negative_zero_the_way_javascript_does() {
        let axis = AxisSpec {
            tag: "TEST".into(),
            name: String::new(),
            low: -0.0,
            default: 0.5,
            high: 144.0,
            affects_metrics: true,
            affects_kerning: false,
        };
        assert!(
            axis.to_key().contains(":0:0.5:144:1:0"),
            "{}",
            axis.to_key()
        );
    }

    /// Keys are read from files, so a stray newline should not be fatal.
    #[test]
    fn ignores_surrounding_whitespace() {
        let key = page_defaults().to_key();
        assert_eq!(
            Options::from_key(&format!("  {key}\n")).unwrap(),
            page_defaults()
        );
    }

    #[test]
    fn rejects_keys_it_cannot_honour() {
        for bad in [
            "",
            "v1",
            "v1;full;;;1;1;extra",
            "v2;full;;;1;1",
            "v1;Full;;;1;1",
            "v1;full;greek+;;1;1",
            "v1;full;klingon;;1;1",
            "v1;full;;wght:Weight:300:400:800:1;1;1",
            "v1;full;;wght:Weight:300:400:800:1:1",
            // A tag that is not four printable ASCII bytes.
            "v1;full;;abc:Small:0:0:100:1:1;1;1",
            "v1;full;;abcde:Small:0:0:100:1:1;1;1",
            "v1;full;;ab\tc:Small:0:0:100:1:1;1;1",
            "v1;full;;ab\u{e7}d:Small:0:0:100:1:1;1;1",
            // Ranges that make no sense.
            "v1;full;;wght:Weight:800:400:300:1:1;1;1",
            "v1;full;;wght:Weight:400:400:400:1:1;1;1",
            "v1;full;;wght:Weight:300:900:800:1:1;1;1",
            "v1;full;;wght:Weight:300:NaN:800:1:1;1;1",
            "v1;full;;wght:Weight:300:inf:800:1:1;1;1",
            "v1;full;;wght:Weight:x:400:800:1:1;1;1",
            "v1;full;;wght:Weight:300:400:800:maybe:1;1;1",
            "v1;full;;wght:Weight:300:400:800:1:1;2;1",
            "v1;full;;wght:Wei%ght:300:400:800:1:1;1;1",
            "v1;full;;wght:Wei%3ght:300:400:800:1:1;1;1",
        ] {
            assert!(Options::from_key(bad).is_err(), "{bad:?} should not parse");
        }
    }

    /// The pipeline the cache builder runs: key in, font out.
    #[test]
    fn builds_the_designspace_the_key_describes() {
        let font = FakeFont::from_key(&page_defaults().to_key()).expect("build");

        // Three positions for wght, wdth and opsz, but ROND's low and default
        // are both zero, so only two for it.
        assert_eq!(font.master_count(), 3 * 3 * 3 * 2);
        assert_eq!(font.glyph_count(), 1158, "full Latin");
    }

    /// A key has to build the font the explicit calls build: same glyphs, same
    /// masters, same tables. The bytes themselves are jittered by `fill_out_masters`
    /// (see `fakeaxis::transform_x_coord`), so they differ run to run.
    #[test]
    fn builds_what_the_same_options_build_by_hand() {
        let mut by_hand = FakeFont::new(LatinCoverage::Full, &[OtherSubsets::Greek]).unwrap();
        by_hand.add_axis("wght", "Weight", 300.0, 800.0, 400.0, true, true);
        by_hand.fill_out_masters(true, true);

        let from_key = FakeFont::from_key("v1;full;greek;wght:Weight:300:400:800:1:1;1;1").unwrap();

        assert_eq!(from_key.master_count(), by_hand.master_count());
        assert_eq!(from_key.glyph_count(), by_hand.glyph_count());

        let key_bytes = from_key.compile().expect("compile the keyed font");
        let hand_bytes = by_hand.compile().expect("compile the hand-built font");
        let keyed_tables = crate::table_stats(&key_bytes).expect("table stats");
        let hand_tables = crate::table_stats(&hand_bytes).expect("table stats");
        let mut keyed: Vec<&str> = keyed_tables.keys().map(String::as_str).collect();
        let mut hand: Vec<&str> = hand_tables.keys().map(String::as_str).collect();
        keyed.sort_unstable();
        hand.sort_unstable();
        assert_eq!(keyed, hand);
        assert!(keyed.contains(&"avar"), "wght should be warped: {keyed:?}");
    }
}

//! The DocIR document model.
//!
//! A [`Document`] is what the engine returns for one input: pages with lines and words, typed
//! fields that point back at the lines and regions that justify them, and a record of the run that
//! produced it. All geometry is in source pixels (origin top-left, y down). The types can be built
//! freely; [`Document::validate`] checks everything that the types themselves cannot express
//! (unique identifiers, references between parts, hexadecimal digests), and serialising a document
//! validates it first, so no invalid document can be written.

use std::collections::{BTreeMap, BTreeSet};

use wanjiru_types::{ErrorKind, Polygon, Result, Size, Source, WnjError, codes};

use crate::confidence::Confidence;
use crate::transform::Chain;

/// The schema identifier written into every document.
pub const SCHEMA_ID: &str = "wanjiru.docir/0";

/// Prefix shared by every version of the schema identifier.
pub const SCHEMA_PREFIX: &str = "wanjiru.docir/";

macro_rules! string_enum {
    ($(#[$meta:meta])* $name:ident { $($(#[$vmeta:meta])* $variant:ident => $text:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
        pub enum $name {
            $($(#[$vmeta])* $variant),+
        }

        impl $name {
            /// Every variant, in declaration order.
            pub const ALL: &'static [$name] = &[$($name::$variant),+];

            /// The spelling used in DocIR JSON.
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $($name::$variant => $text),+
                }
            }

            /// Read the JSON spelling.
            #[must_use]
            pub fn parse(text: &str) -> Option<Self> {
                match text {
                    $($text => Some($name::$variant),)+
                    _ => None,
                }
            }
        }
    };
}

string_enum!(
    /// The kind of document, which selects the quality contract it is judged by.
    DocClass {
        /// A commercial invoice.
        Invoice => "invoice",
        /// A till receipt.
        Receipt => "receipt",
        /// An identity document with a machine-readable zone.
        Id => "id",
        /// Anything else.
        Other => "other",
    }
);

string_enum!(
    /// The kind of value a field holds, which selects the syntax its value follows.
    FieldKind {
        /// A monetary amount as a canonical decimal string, such as `1234.50`.
        Amount => "amount",
        /// A calendar date as `YYYY-MM-DD`.
        Date => "date",
        /// An IBAN without spaces, in upper case.
        Iban => "iban",
        /// The lines of a machine-readable zone, joined by `\n`.
        Mrz => "mrz",
        /// Free text.
        Text => "text",
    }
);

string_enum!(
    /// What the engine decided to do with a field.
    Decision {
        /// The value is confident enough to be used without a human look.
        Accept => "accept",
        /// The value should be reviewed by a person.
        Review => "review",
    }
);

/// What the document was read from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceInfo {
    /// SHA-256 of the input bytes, 64 lower-case hexadecimal digits.
    pub sha256: String,
    /// Media type of the input, such as `image/png` or `application/pdf`.
    pub media_type: String,
}

/// A word with its region.
#[derive(Clone, Debug, PartialEq)]
pub struct Word {
    /// The recognised text.
    pub text: String,
    /// The region, in source pixels.
    pub polygon: Polygon<Source>,
    /// Confidence in the text.
    pub confidence: Confidence,
}

/// A line of text with its region and words.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    /// Identifier, unique within the document; fields refer to lines by it.
    pub id: String,
    /// The recognised text.
    pub text: String,
    /// The region, in source pixels.
    pub polygon: Polygon<Source>,
    /// Confidence in the text.
    pub confidence: Confidence,
    /// The words of the line, possibly empty if the engine did not segment it.
    pub words: Vec<Word>,
}

/// One page.
#[derive(Clone, Debug, PartialEq)]
pub struct Page {
    /// Zero-based position in the document; pages are numbered consecutively.
    pub index: u32,
    /// Size of the source image in pixels.
    pub size: Size<Source>,
    /// How the source image was mapped to the frames the models saw. Starts in the source frame.
    pub chain: Chain,
    /// The lines, in reading order.
    pub lines: Vec<Line>,
}

/// A region and the text read there, offered as the justification of a field.
#[derive(Clone, Debug, PartialEq)]
pub struct Evidence {
    /// Index of the page the region is on.
    pub page: u32,
    /// The region, in source pixels.
    pub polygon: Polygon<Source>,
    /// The text read in the region.
    pub text: String,
}

/// A typed value extracted from the document.
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    /// Name, unique within the document, such as `total_amount`.
    pub name: String,
    /// The kind of value.
    pub kind: FieldKind,
    /// The value in its canonical form (see [`FieldKind`]); never empty.
    pub value: String,
    /// ISO 4217 code of the currency, for amounts only.
    pub currency: Option<String>,
    /// Confidence in the value.
    pub confidence: Confidence,
    /// Accept or send to review.
    pub decision: Decision,
    /// Regions that justify the value.
    pub evidence: Vec<Evidence>,
    /// Identifiers of the lines the value was derived from.
    pub lines: Vec<String>,
}

/// A record of the run that produced the document.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    /// Name and version of the engine.
    pub engine: String,
    /// SHA-256 of the canonical configuration, 64 lower-case hexadecimal digits.
    pub config_sha256: String,
    /// Wall-clock milliseconds by stage. Excluded from [`Document::content_sha256`].
    pub timing_ms: BTreeMap<String, f64>,
}

/// The result of reading one document.
#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    /// Identifier, unique within a corpus.
    pub id: String,
    /// The kind of document.
    pub class: DocClass,
    /// What it was read from.
    pub source: SourceInfo,
    /// The pages.
    pub pages: Vec<Page>,
    /// The extracted fields.
    pub fields: Vec<Field>,
    /// How it was produced.
    pub run: Run,
}

fn violation(path: &str, message: impl Into<String>) -> WnjError {
    WnjError::new(codes::SCHEMA_VIOLATION, ErrorKind::InvalidInput, message).at(path)
}

fn is_lower_hex(text: &str, length: usize) -> bool {
    text.len() == length && text.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn check_identifier(path: &str, what: &str, id: &str) -> Result<()> {
    if id.is_empty() || id.chars().count() > 64 {
        return Err(violation(
            path,
            format!("{what} must have 1 to 64 characters"),
        ));
    }
    Ok(())
}

impl Document {
    /// Check everything the types cannot enforce; the error names the first problem and its location
    /// as a JSON Pointer into the serialised form.
    pub fn validate(&self) -> Result<()> {
        if self.id.is_empty() || self.id.chars().count() > 256 {
            return Err(violation(
                "/id",
                "the document id must have 1 to 256 characters",
            ));
        }
        if !is_lower_hex(&self.source.sha256, 64) {
            return Err(violation(
                "/source/sha256",
                "must be 64 lower-case hexadecimal digits",
            ));
        }
        if self.source.media_type.len() < 3 || self.source.media_type.len() > 127 {
            return Err(violation(
                "/source/media_type",
                "must have 3 to 127 characters",
            ));
        }
        let line_ids = self.validate_pages()?;
        self.validate_fields(&line_ids)?;
        self.validate_run()
    }

    fn validate_pages(&self) -> Result<BTreeSet<&str>> {
        let mut line_ids = BTreeSet::new();
        for (p, page) in self.pages.iter().enumerate() {
            let here = format!("/pages/{p}");
            if usize::try_from(page.index) != Ok(p) {
                return Err(violation(
                    &format!("{here}/index"),
                    format!(
                        "pages are numbered consecutively from 0: expected {p}, found {}",
                        page.index
                    ),
                ));
            }
            let (w, h) = (page.size.w, page.size.h);
            if !(w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0) {
                return Err(violation(
                    &format!("{here}/size"),
                    "width and height must be finite and positive",
                ));
            }
            if page.chain.from_frame() != "source" {
                return Err(violation(
                    &format!("{here}/chain/from"),
                    format!(
                        "a page chain starts in the source frame, not {:?}",
                        page.chain.from_frame()
                    ),
                ));
            }
            for (l, line) in page.lines.iter().enumerate() {
                let line_path = format!("{here}/lines/{l}");
                check_identifier(&format!("{line_path}/id"), "a line id", &line.id)?;
                if !line_ids.insert(line.id.as_str()) {
                    return Err(violation(
                        &format!("{line_path}/id"),
                        format!("line id {:?} is used twice", line.id),
                    ));
                }
            }
        }
        Ok(line_ids)
    }

    fn validate_fields(&self, line_ids: &BTreeSet<&str>) -> Result<()> {
        let mut names = BTreeSet::new();
        for (f, field) in self.fields.iter().enumerate() {
            let here = format!("/fields/{f}");
            check_identifier(&format!("{here}/name"), "a field name", &field.name)?;
            if !names.insert(field.name.as_str()) {
                return Err(violation(
                    &format!("{here}/name"),
                    format!("field name {:?} is used twice", field.name),
                ));
            }
            if field.value.is_empty() {
                return Err(violation(
                    &format!("{here}/value"),
                    "a field value is never empty; leave an absent field out",
                ));
            }
            match (&field.currency, field.kind) {
                (Some(code), FieldKind::Amount) => {
                    if code.len() != 3 || !code.bytes().all(|b| b.is_ascii_uppercase()) {
                        return Err(violation(
                            &format!("{here}/currency"),
                            "must be three upper-case ASCII letters",
                        ));
                    }
                }
                (Some(_), _) => {
                    return Err(violation(
                        &format!("{here}/currency"),
                        "only amount fields carry a currency",
                    ));
                }
                (None, _) => {}
            }
            for (e, evidence) in field.evidence.iter().enumerate() {
                if usize::try_from(evidence.page).map_or(true, |p| p >= self.pages.len()) {
                    return Err(violation(
                        &format!("{here}/evidence/{e}/page"),
                        format!(
                            "page {} does not exist (the document has {})",
                            evidence.page,
                            self.pages.len()
                        ),
                    ));
                }
            }
            for (l, id) in field.lines.iter().enumerate() {
                if !line_ids.contains(id.as_str()) {
                    return Err(violation(
                        &format!("{here}/lines/{l}"),
                        format!("no line has the id {id:?}"),
                    ));
                }
            }
        }
        Ok(())
    }

    fn validate_run(&self) -> Result<()> {
        if self.run.engine.is_empty() || self.run.engine.chars().count() > 128 {
            return Err(violation("/run/engine", "must have 1 to 128 characters"));
        }
        if !is_lower_hex(&self.run.config_sha256, 64) {
            return Err(violation(
                "/run/config_sha256",
                "must be 64 lower-case hexadecimal digits",
            ));
        }
        for (stage, ms) in &self.run.timing_ms {
            if !(ms.is_finite() && *ms >= 0.0) {
                return Err(violation(
                    &format!(
                        "/run/timing_ms/{}",
                        stage.replace('~', "~0").replace('/', "~1")
                    ),
                    "a timing is a finite, non-negative number of milliseconds",
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wanjiru_types::Point;

    pub(crate) fn square(x: f32, y: f32, side: f32) -> Polygon<Source> {
        Polygon::new(vec![
            Point::new(x, y),
            Point::new(x + side, y),
            Point::new(x + side, y + side),
            Point::new(x, y + side),
        ])
        .expect("valid polygon")
    }

    pub(crate) fn sample() -> Document {
        let line = |id: &str, text: &str, y: f32| Line {
            id: id.to_owned(),
            text: text.to_owned(),
            polygon: square(10.0, y, 20.0),
            confidence: Confidence::score(0.97).expect("in range"),
            words: Vec::new(),
        };
        Document {
            id: "inv-0001".to_owned(),
            class: DocClass::Invoice,
            source: SourceInfo {
                sha256: "ab".repeat(32),
                media_type: "image/png".to_owned(),
            },
            pages: vec![Page {
                index: 0,
                size: Size::new(1000.0, 1400.0),
                chain: Chain::new("source").expect("known frame"),
                lines: vec![
                    line("l0", "Invoice 2026-001", 10.0),
                    line("l1", "Total 1 234,50 EUR", 40.0),
                ],
            }],
            fields: vec![Field {
                name: "total_amount".to_owned(),
                kind: FieldKind::Amount,
                value: "1234.50".to_owned(),
                currency: Some("EUR".to_owned()),
                confidence: Confidence::score(0.99).expect("in range"),
                decision: Decision::Accept,
                evidence: vec![Evidence {
                    page: 0,
                    polygon: square(10.0, 40.0, 20.0),
                    text: "1 234,50".to_owned(),
                }],
                lines: vec!["l1".to_owned()],
            }],
            run: Run {
                engine: "wanjiru 0.1.0".to_owned(),
                config_sha256: "cd".repeat(32),
                timing_ms: BTreeMap::from([("total".to_owned(), 12.5)]),
            },
        }
    }

    fn rejected(mutate: impl FnOnce(&mut Document)) -> WnjError {
        let mut doc = sample();
        mutate(&mut doc);
        doc.validate().expect_err("should be invalid")
    }

    #[test]
    fn the_sample_is_valid() {
        sample().validate().expect("valid");
    }

    #[test]
    fn enums_round_trip_through_their_spelling() {
        for class in DocClass::ALL {
            assert_eq!(DocClass::parse(class.as_str()), Some(*class));
        }
        for kind in FieldKind::ALL {
            assert_eq!(FieldKind::parse(kind.as_str()), Some(*kind));
        }
        for decision in Decision::ALL {
            assert_eq!(Decision::parse(decision.as_str()), Some(*decision));
        }
        assert_eq!(DocClass::parse("Invoice"), None);
        assert_eq!(FieldKind::parse(""), None);
        assert_eq!(FieldKind::ALL.len(), 5);
    }

    #[test]
    fn structural_problems_are_located() {
        type Mutation = Box<dyn FnOnce(&mut Document)>;
        let cases: Vec<(&str, Mutation)> = vec![
            ("/id", Box::new(|d| d.id.clear())),
            (
                "/source/sha256",
                Box::new(|d| d.source.sha256 = "AB".repeat(32)),
            ),
            (
                "/source/sha256",
                Box::new(|d| d.source.sha256 = "ab".repeat(31)),
            ),
            ("/pages/0/index", Box::new(|d| d.pages[0].index = 1)),
            (
                "/pages/0/size",
                Box::new(|d| d.pages[0].size = Size::new(0.0, 10.0)),
            ),
            (
                "/pages/0/size",
                Box::new(|d| d.pages[0].size = Size::new(f32::NAN, 10.0)),
            ),
            (
                "/pages/0/chain/from",
                Box::new(|d| d.pages[0].chain = Chain::new("page").expect("known")),
            ),
            (
                "/pages/0/lines/1/id",
                Box::new(|d| d.pages[0].lines[1].id = "l0".to_owned()),
            ),
            (
                "/pages/0/lines/0/id",
                Box::new(|d| d.pages[0].lines[0].id.clear()),
            ),
            ("/fields/0/name", Box::new(|d| d.fields[0].name.clear())),
            ("/fields/0/value", Box::new(|d| d.fields[0].value.clear())),
            (
                "/fields/0/currency",
                Box::new(|d| d.fields[0].currency = Some("eur".to_owned())),
            ),
            (
                "/fields/0/currency",
                Box::new(|d| d.fields[0].kind = FieldKind::Text),
            ),
            (
                "/fields/0/evidence/0/page",
                Box::new(|d| d.fields[0].evidence[0].page = 1),
            ),
            (
                "/fields/0/lines/0",
                Box::new(|d| d.fields[0].lines[0] = "nope".to_owned()),
            ),
            ("/run/engine", Box::new(|d| d.run.engine.clear())),
            (
                "/run/config_sha256",
                Box::new(|d| d.run.config_sha256 = "zz".repeat(32)),
            ),
            (
                "/run/timing_ms/total",
                Box::new(|d| {
                    d.run.timing_ms.insert("total".to_owned(), -1.0);
                }),
            ),
        ];
        for (expected_path, mutate) in cases {
            let error = rejected(mutate);
            assert_eq!(error.path(), Some(expected_path), "{error}");
            assert_eq!(error.code().to_string(), "WNJ-E1003");
        }
    }

    #[test]
    fn duplicate_field_names_are_refused() {
        let error = rejected(|d| {
            let copy = d.fields[0].clone();
            d.fields.push(copy);
        });
        assert_eq!(error.path(), Some("/fields/1/name"));
    }
}

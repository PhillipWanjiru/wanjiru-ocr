//! Quality contracts: what "good enough" means for one class of documents, written as data.
//!
//! A contract is a small YAML file (see `eval/contracts/`). It names a class of documents and a frozen
//! corpus, lists the thresholds the engine must meet, and the harness turns every threshold into a
//! gate with a three-valued verdict. The reader is strict: an unknown key, a wrong type or a value
//! out of range is an error with a path, because a contract that silently ignores a misspelt
//! threshold is worse than no contract.
//!
//! ```yaml
//! contract: invoice-fields-v1
//! class: invoice
//! status: draft                      # draft until gate G1 freezes it
//! corpus: golden/invoices-v0
//! metrics:
//!   field_exact_match:
//!     total_amount: {kind: amount, min: 0.995, coverage: 0.95}
//!   ece_char: {max: 0.02}            # measured from Phase 6
//!   conformal: {alpha: 0.01, coverage_tolerance: 0.01}
//!   latency_ms: {p50_max: 1000, p95_max: 2500}
//! gates:
//!   regression_budget_cer_rel: 0.02  # block a change that worsens CER by more than 2 % (relative)
//!   confidence_interval: bootstrap-by-document
//! ```
//!
//! `min` is the required accuracy among the fields the engine *accepts*; `coverage` is the share of
//! documents for which the engine must accept the field at all. A field gate without a `kind` takes
//! the kind from its name when the name is one of the kinds (`amount`, `date`, `iban`, `mrz`, `text`).

use wanjiru_docir::{DocClass, FieldKind, Json};
use wanjiru_types::{ErrorKind, Result, WnjError, codes};

use crate::yaml;

/// How final a contract is.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Status {
    /// Thresholds are proposals; the report says so.
    Draft,
    /// Thresholds were confirmed at a gate and changing them needs an architecture decision record.
    Frozen,
}

impl Status {
    /// The spelling used in contracts and reports.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Frozen => "frozen",
        }
    }
}

/// The thresholds for one extracted field.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldGate {
    /// The field name, as it appears in truth and predictions.
    pub name: String,
    /// The syntax of the value.
    pub kind: FieldKind,
    /// Required accuracy among accepted values (lower end of the interval must reach it).
    pub min: f64,
    /// Required share of documents in which the field is accepted.
    pub coverage: f64,
}

/// The conformal-prediction requirement; measured from Phase 6.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Conformal {
    /// Target miscoverage.
    pub alpha: f64,
    /// Allowed deviation of empirical coverage from `1 - alpha`.
    pub coverage_tolerance: f64,
}

/// A parsed quality contract.
#[derive(Clone, Debug, PartialEq)]
pub struct Contract {
    /// Identifier, such as `invoice-fields-v1`.
    pub id: String,
    /// The class of documents it applies to.
    pub class: DocClass,
    /// Draft or frozen.
    pub status: Status,
    /// Name of the corpus the thresholds refer to.
    pub corpus: String,
    /// Field gates, in name order.
    pub fields: Vec<FieldGate>,
    /// Upper bound on the expected calibration error of character confidences (Phase 6).
    pub ece_char_max: Option<f64>,
    /// Conformal coverage requirement (Phase 6).
    pub conformal: Option<Conformal>,
    /// Upper bound on median latency per document, in milliseconds.
    pub latency_p50_max: Option<f64>,
    /// Upper bound on the 95th-percentile latency per document, in milliseconds.
    pub latency_p95_max: Option<f64>,
    /// Largest tolerated relative increase of the character error rate against the baseline.
    pub regression_budget_cer_rel: Option<f64>,
}

fn invalid(path: &str, message: impl Into<String>) -> WnjError {
    WnjError::new(codes::CONTRACT_INVALID, ErrorKind::InvalidInput, message).at(path)
}

/// An object whose keys are all accounted for before it is dropped.
struct Map<'a> {
    path: String,
    members: &'a [(String, Json)],
    used: Vec<&'static str>,
}

impl<'a> Map<'a> {
    fn new(value: &'a Json, path: &str) -> Result<Self> {
        match value.as_object() {
            Some(members) => Ok(Self {
                path: path.to_owned(),
                members,
                used: Vec::new(),
            }),
            None => Err(invalid(
                path,
                format!("expected a mapping, found {}", value.type_name()),
            )),
        }
    }

    fn child(&self, key: &str) -> String {
        format!("{}/{key}", self.path)
    }

    fn optional(&mut self, key: &'static str) -> Option<&'a Json> {
        self.used.push(key);
        self.members.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    fn required(&mut self, key: &'static str) -> Result<&'a Json> {
        self.optional(key)
            .ok_or_else(|| invalid(&self.path, format!("missing required key `{key}`")))
    }

    fn text(&mut self, key: &'static str) -> Result<&'a str> {
        let value = self.required(key)?;
        value.as_str().ok_or_else(|| {
            invalid(
                &self.child(key),
                format!("expected a string, found {}", value.type_name()),
            )
        })
    }

    fn number(&mut self, key: &'static str) -> Result<f64> {
        let value = self.required(key)?;
        self.as_number(key, value)
    }

    fn optional_number(&mut self, key: &'static str) -> Result<Option<f64>> {
        match self.optional(key) {
            Some(value) => self.as_number(key, value).map(Some),
            None => Ok(None),
        }
    }

    fn as_number(&self, key: &str, value: &Json) -> Result<f64> {
        match value.as_f64() {
            Some(x) if x.is_finite() => Ok(x),
            _ => Err(invalid(
                &self.child(key),
                format!("expected a number, found {}", value.type_name()),
            )),
        }
    }

    /// Fail on any key that was never asked for.
    fn finish(self) -> Result<()> {
        let unknown: Vec<&str> = self
            .members
            .iter()
            .map(|(k, _)| k.as_str())
            .filter(|k| !self.used.contains(k))
            .collect();
        if unknown.is_empty() {
            Ok(())
        } else {
            let plural = if unknown.len() == 1 { "" } else { "s" };
            Err(invalid(
                &self.path,
                format!(
                    "unknown key{plural} {}; known keys: {}",
                    unknown
                        .iter()
                        .map(|k| format!("`{k}`"))
                        .collect::<Vec<_>>()
                        .join(", "),
                    self.used
                        .iter()
                        .map(|k| format!("`{k}`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            ))
        }
    }
}

fn in_range(
    path: &str,
    what: &str,
    x: f64,
    low_open: bool,
    high_open: bool,
    low: f64,
    high: f64,
) -> Result<f64> {
    let above = if low_open { x > low } else { x >= low };
    let below = if high_open { x < high } else { x <= high };
    if above && below {
        Ok(x)
    } else {
        Err(invalid(
            path,
            format!(
                "{what} must be in {}{low}, {high}{}, found {x}",
                if low_open { '(' } else { '[' },
                if high_open { ')' } else { ']' }
            ),
        ))
    }
}

fn identifier(path: &str, what: &str, text: &str, allow_slash: bool) -> Result<()> {
    let good = !text.is_empty()
        && text.len() <= 64
        && text.bytes().all(|b| {
            b.is_ascii_alphanumeric()
                || matches!(b, b'-' | b'_' | b'.')
                || (allow_slash && b == b'/')
        });
    if good {
        Ok(())
    } else {
        Err(invalid(
            path,
            format!(
                "{what} is 1 to 64 characters of letters, digits, `-`, `_` and `.`{}",
                if allow_slash { " (and `/`)" } else { "" }
            ),
        ))
    }
}

fn field_gate(name: &str, value: &Json, path: &str) -> Result<FieldGate> {
    if !(name.len() <= 64
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'))
    {
        return Err(invalid(
            path,
            "a field name is a lower-case letter followed by lower-case letters, digits and `_` (at most 64)",
        ));
    }
    let mut map = Map::new(value, path)?;
    let kind = match map.optional("kind") {
        Some(Json::String(text)) => FieldKind::parse(text).ok_or_else(|| {
            invalid(&map.child("kind"), format!("unknown kind `{text}`; kinds are amount, date, iban, mrz, text"))
        })?,
        Some(other) => {
            return Err(invalid(
                &map.child("kind"),
                format!("expected a string, found {}", other.type_name()),
            ));
        }
        None => FieldKind::parse(name).ok_or_else(|| {
            invalid(
                path,
                format!("the field `{name}` needs a `kind`: only a field named after its kind may leave it out"),
            )
        })?,
    };
    let min = map.number("min")?;
    in_range(&map.child("min"), "`min`", min, true, false, 0.0, 1.0)?;
    let coverage = map.number("coverage")?;
    in_range(
        &map.child("coverage"),
        "`coverage`",
        coverage,
        true,
        false,
        0.0,
        1.0,
    )?;
    map.finish()?;
    Ok(FieldGate {
        name: name.to_owned(),
        kind,
        min,
        coverage,
    })
}

impl Contract {
    /// Parse and validate a contract from YAML text.
    ///
    /// # Errors
    ///
    /// `WNJ-E4001` if the text is not in the supported YAML subset, `WNJ-E4002` if it is not a valid
    /// contract.
    pub fn from_yaml(text: &str) -> Result<Self> {
        Self::from_json(&yaml::parse(text)?)
    }

    /// Validate a contract given as a JSON value (the YAML reader's output).
    ///
    /// # Errors
    ///
    /// `WNJ-E4002` if the value is not a valid contract.
    pub fn from_json(value: &Json) -> Result<Self> {
        let mut top = Map::new(value, "")?;
        let id = top.text("contract")?;
        identifier("/contract", "the contract name", id, false)?;
        let class_name = top.text("class")?;
        let class = DocClass::parse(class_name).ok_or_else(|| {
            invalid(
                "/class",
                format!("unknown class `{class_name}`; classes are invoice, receipt, id, other"),
            )
        })?;
        let status = match top.optional("status") {
            None => Status::Draft,
            Some(Json::String(s)) if s == "draft" => Status::Draft,
            Some(Json::String(s)) if s == "frozen" => Status::Frozen,
            Some(_) => return Err(invalid("/status", "`status` is `draft` or `frozen`")),
        };
        let corpus = top.text("corpus")?;
        identifier("/corpus", "the corpus name", corpus, true)?;

        let metrics_value = top.required("metrics")?;
        let mut metrics = Map::new(metrics_value, "/metrics")?;
        let mut contract = Self {
            id: id.to_owned(),
            class,
            status,
            corpus: corpus.to_owned(),
            fields: Vec::new(),
            ece_char_max: None,
            conformal: None,
            latency_p50_max: None,
            latency_p95_max: None,
            regression_budget_cer_rel: None,
        };

        if let Some(fields) = metrics.optional("field_exact_match") {
            let path = metrics.child("field_exact_match");
            let members = fields.as_object().ok_or_else(|| {
                invalid(
                    &path,
                    format!("expected a mapping, found {}", fields.type_name()),
                )
            })?;
            for (name, gate) in members {
                contract
                    .fields
                    .push(field_gate(name, gate, &format!("{path}/{name}"))?);
            }
            contract.fields.sort_by(|a, b| a.name.cmp(&b.name));
        }
        if let Some(ece) = metrics.optional("ece_char") {
            let mut map = Map::new(ece, &metrics.child("ece_char"))?;
            let max = map.number("max")?;
            contract.ece_char_max = Some(in_range(
                &map.child("max"),
                "`max`",
                max,
                true,
                false,
                0.0,
                1.0,
            )?);
            map.finish()?;
        }
        if let Some(conformal) = metrics.optional("conformal") {
            let mut map = Map::new(conformal, &metrics.child("conformal"))?;
            let alpha = map.number("alpha")?;
            in_range(&map.child("alpha"), "`alpha`", alpha, true, true, 0.0, 1.0)?;
            let tolerance = map.number("coverage_tolerance")?;
            in_range(
                &map.child("coverage_tolerance"),
                "`coverage_tolerance`",
                tolerance,
                false,
                true,
                0.0,
                1.0,
            )?;
            map.finish()?;
            contract.conformal = Some(Conformal {
                alpha,
                coverage_tolerance: tolerance,
            });
        }
        if let Some(latency) = metrics.optional("latency_ms") {
            let mut map = Map::new(latency, &metrics.child("latency_ms"))?;
            for (key, slot) in [
                ("p50_max", &mut contract.latency_p50_max),
                ("p95_max", &mut contract.latency_p95_max),
            ] {
                if let Some(x) = map.optional_number(key)? {
                    if x <= 0.0 {
                        return Err(invalid(
                            &map.child(key),
                            format!("`{key}` must be positive, found {x}"),
                        ));
                    }
                    *slot = Some(x);
                }
            }
            if contract.latency_p50_max.is_none() && contract.latency_p95_max.is_none() {
                return Err(invalid(&map.path, "give `p50_max`, `p95_max` or both"));
            }
            if let (Some(p50), Some(p95)) = (contract.latency_p50_max, contract.latency_p95_max) {
                if p50 > p95 {
                    return Err(invalid(&map.path, "`p50_max` must not exceed `p95_max`"));
                }
            }
            map.finish()?;
        }
        metrics.finish()?;

        let gates_value = top.required("gates")?;
        let mut gates = Map::new(gates_value, "/gates")?;
        if let Some(budget) = gates.optional_number("regression_budget_cer_rel")? {
            if budget < 0.0 {
                return Err(invalid(
                    &gates.child("regression_budget_cer_rel"),
                    format!("the budget is not negative, found {budget}"),
                ));
            }
            contract.regression_budget_cer_rel = Some(budget);
        }
        match gates.optional("confidence_interval") {
            None => {}
            Some(Json::String(s)) if s == "bootstrap-by-document" => {}
            Some(_) => {
                return Err(invalid(
                    &gates.child("confidence_interval"),
                    "`confidence_interval` is `bootstrap-by-document`, the only method this version knows",
                ));
            }
        }
        gates.finish()?;
        top.finish()?;

        if contract.gate_count() == 0 {
            return Err(invalid("", "a contract states at least one threshold"));
        }
        Ok(contract)
    }

    /// The number of thresholds the contract states (two per field, one per other metric).
    #[must_use]
    pub fn gate_count(&self) -> usize {
        self.fields.len() * 2
            + usize::from(self.ece_char_max.is_some())
            + usize::from(self.conformal.is_some())
            + usize::from(self.latency_p50_max.is_some())
            + usize::from(self.latency_p95_max.is_some())
            + usize::from(self.regression_budget_cer_rel.is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLUEPRINT: &str = "\
contract: invoice-fields-v1
class: invoice
corpus: golden/invoices-2026-q3        # frozen, versioned, document-level split
metrics:
  field_exact_match:
    amount: {min: 0.995, coverage: 0.95}
    date:   {min: 0.995, coverage: 0.95}
    iban:   {min: 0.999, coverage: 0.90}
  ece_char: {max: 0.02}
  conformal: {alpha: 0.01, coverage_tolerance: 0.01}
  latency_ms: {p50_max: 1000, p95_max: 2500}
gates:
  regression_budget_cer_rel: 0.02       # block merge if CER worsens by more than 2% relative
  confidence_interval: bootstrap-by-document
";

    fn error(text: &str) -> WnjError {
        Contract::from_yaml(text).expect_err("should be rejected")
    }

    fn with(replacement: (&str, &str)) -> String {
        assert!(BLUEPRINT.contains(replacement.0), "{}", replacement.0);
        BLUEPRINT.replacen(replacement.0, replacement.1, 1)
    }

    #[test]
    fn the_blueprint_example_is_a_valid_contract() {
        let c = Contract::from_yaml(BLUEPRINT).expect("valid");
        assert_eq!(c.id, "invoice-fields-v1");
        assert_eq!(c.class, DocClass::Invoice);
        assert_eq!(c.status, Status::Draft);
        assert_eq!(c.corpus, "golden/invoices-2026-q3");
        let names: Vec<_> = c.fields.iter().map(|f| (f.name.as_str(), f.kind)).collect();
        assert_eq!(
            names,
            [
                ("amount", FieldKind::Amount),
                ("date", FieldKind::Date),
                ("iban", FieldKind::Iban)
            ]
        );
        assert!(
            (c.fields[2].min - 0.999).abs() < 1e-15 && (c.fields[2].coverage - 0.9).abs() < 1e-15
        );
        assert_eq!(c.ece_char_max, Some(0.02));
        assert_eq!(
            c.conformal,
            Some(Conformal {
                alpha: 0.01,
                coverage_tolerance: 0.01
            })
        );
        assert_eq!(
            (c.latency_p50_max, c.latency_p95_max),
            (Some(1000.0), Some(2500.0))
        );
        assert_eq!(c.regression_budget_cer_rel, Some(0.02));
        assert_eq!(c.gate_count(), 6 + 1 + 1 + 2 + 1);
    }

    #[test]
    fn renamed_fields_name_their_kind() {
        let text = with((
            "    amount: {min: 0.995, coverage: 0.95}",
            "    total_amount: {kind: amount, min: 0.995, coverage: 0.95}",
        ));
        let c = Contract::from_yaml(&text).expect("valid");
        assert_eq!(c.fields[2].name, "total_amount");
        let text = with((
            "    amount: {min: 0.995, coverage: 0.95}",
            "    total_amount: {min: 0.995, coverage: 0.95}",
        ));
        let e = error(&text);
        assert_eq!(e.code(), codes::CONTRACT_INVALID);
        assert!(e.message().contains("needs a `kind`"), "{e}");
    }

    #[test]
    fn every_mistake_has_a_path_and_a_code() {
        let cases: [((&str, &str), &str); 12] = [
            (
                ("contract: invoice-fields-v1", "contract: 'bad name'"),
                "/contract",
            ),
            (("class: invoice", "class: invoce"), "/class"),
            (
                (
                    "min: 0.995, coverage: 0.95}\n    date",
                    "min: 1.5, coverage: 0.95}\n    date",
                ),
                "/metrics/field_exact_match/amount/min",
            ),
            (
                (
                    "iban:   {min: 0.999, coverage: 0.90}",
                    "iban:   {min: 0.999}",
                ),
                "/metrics/field_exact_match/iban",
            ),
            (
                (
                    "iban:   {min: 0.999, coverage: 0.90}",
                    "iban:   {min: 0.999, coverage: 0.9, extra: 1}",
                ),
                "/metrics/field_exact_match/iban",
            ),
            (
                ("ece_char: {max: 0.02}", "ece_char: {max: high}"),
                "/metrics/ece_char/max",
            ),
            (("alpha: 0.01", "alpha: 0"), "/metrics/conformal/alpha"),
            (
                (
                    "p50_max: 1000, p95_max: 2500",
                    "p50_max: 3000, p95_max: 2500",
                ),
                "/metrics/latency_ms",
            ),
            (
                (
                    "regression_budget_cer_rel: 0.02",
                    "regression_budget_cer_rel: -0.02",
                ),
                "/gates/regression_budget_cer_rel",
            ),
            (
                ("bootstrap-by-document", "jackknife"),
                "/gates/confidence_interval",
            ),
            (("metrics:", "metricz:"), ""),
            (("corpus: golden/invoices-2026-q3", "corpus: ''"), "/corpus"),
        ];
        for ((from, to), path) in cases {
            let e = error(&with((from, to)));
            assert_eq!(e.code(), codes::CONTRACT_INVALID, "{from} -> {to}: {e}");
            assert_eq!(e.path().unwrap_or(""), path, "{from} -> {to}: {e}");
        }
    }

    #[test]
    fn unknown_keys_are_listed() {
        let e = error(&with(("gates:", "extras: 1\ngates:")));
        assert!(e.message().contains("`extras`"), "{e}");
        let e = error(&format!("{BLUEPRINT}stray: 1\nother: 2\n"));
        assert!(
            e.message().contains("`stray`") && e.message().contains("`other`"),
            "{e}"
        );
    }

    #[test]
    fn a_contract_needs_a_threshold_and_valid_yaml() {
        let e = error("contract: x\nclass: other\ncorpus: c\nmetrics: {}\ngates: {}\n");
        assert!(e.message().contains("at least one threshold"), "{e}");
        let e = error("contract: [unclosed\n");
        assert_eq!(e.code(), codes::CONTRACT_PARSE);
        let e = error("- a list\n");
        assert_eq!(e.code(), codes::CONTRACT_INVALID);
    }

    #[test]
    fn status_may_be_frozen() {
        let c = Contract::from_yaml(&with(("class: invoice", "class: invoice\nstatus: frozen")))
            .expect("valid");
        assert_eq!(c.status, Status::Frozen);
        let e = error(&with(("class: invoice", "class: invoice\nstatus: final")));
        assert_eq!(e.path(), Some("/status"));
    }
}

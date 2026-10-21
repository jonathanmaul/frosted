//! Canonical hashing (docs/PROJECT_SPEC_FROSTED.md §19.3, ADR-0005): JCS (RFC 8785) for the
//! subset object/array/string/integer/bool/null, `rules_hash`, `plan_hash`,
//! `receiver_commitment`, and `note_ref`.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::OrchardReceiver;
use crate::blake2b_256;
use crate::params::{PERSONAL_NOTE_REF, PERSONAL_PLAN, PERSONAL_RCVR, PERSONAL_RULES};

/// Every integer in a hashed document must be below 2^53 (total ZEC supply is 2.1e15 zat).
pub const MAX_SAFE_INTEGER: u64 = (1 << 53) - 1;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum HashError {
    #[error("floats are not allowed in hashed documents")]
    Float,
    #[error("integer {0} is outside ±(2^53 - 1)")]
    UnsafeInteger(String),
    #[error("invalid document: {0}")]
    Invalid(String),
}

// ---------------------------------------------------------------------------------------------
// JCS
// ---------------------------------------------------------------------------------------------

/// RFC 8785 canonical serialization of a JSON value in the supported subset.
pub fn jcs(value: &Value) -> Result<String, HashError> {
    let mut out = String::new();
    write_value(value, &mut out)?;
    Ok(out)
}

fn write_value(value: &Value, out: &mut String) -> Result<(), HashError> {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => {
            if let Some(u) = n.as_u64() {
                if u > MAX_SAFE_INTEGER {
                    return Err(HashError::UnsafeInteger(u.to_string()));
                }
                out.push_str(&u.to_string());
            } else if let Some(i) = n.as_i64() {
                if i.unsigned_abs() > MAX_SAFE_INTEGER {
                    return Err(HashError::UnsafeInteger(i.to_string()));
                }
                out.push_str(&i.to_string());
            } else {
                return Err(HashError::Float);
            }
        }
        Value::String(s) => write_string(s, out),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(item, out)?;
            }
            out.push(']');
        }
        Value::Object(map) => {
            // RFC 8785 §3.2.3: sort members by their names as arrays of UTF-16 code units.
            let mut members: Vec<(&String, &Value)> = map.iter().collect();
            members.sort_by(|(a, _), (b, _)| a.encode_utf16().cmp(b.encode_utf16()));
            out.push('{');
            for (i, (key, item)) in members.into_iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(key, out);
                out.push(':');
                write_value(item, out)?;
            }
            out.push('}');
        }
    }
    Ok(())
}

/// RFC 8785 §3.2.2.2 (ECMAScript `JSON.stringify`): escape `"`, `\` and controls below U+0020;
/// everything else is emitted as UTF-8.
fn write_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn hash_document<T: Serialize>(personal: &[u8; 16], doc: &T) -> Result<[u8; 32], HashError> {
    let value = serde_json::to_value(doc).map_err(|e| HashError::Invalid(e.to_string()))?;
    let canonical = jcs(&value)?;
    Ok(blake2b_256(personal, None, &[canonical.as_bytes()]))
}

// ---------------------------------------------------------------------------------------------
// market_definition and rules_hash
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutcomeDefinition {
    pub outcome_index: u32,
    pub label: String,
    pub address: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResolutionType {
    PoolThreshold,
    UpgradeActive,
    Committee,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PoolThresholdParams {
    pub pool: String,
    pub comparator: String,
    pub threshold_zat: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpgradeActiveParams {
    pub upgrade_name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommitteeParams {
    pub criteria: String,
    pub evidence_source: String,
    pub dispute_window_blocks: u32,
    pub void_after_blocks: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ResolutionParams {
    PoolThreshold(PoolThresholdParams),
    UpgradeActive(UpgradeActiveParams),
    Committee(CommitteeParams),
}

/// The JCS input of `rules_hash`. All parameters a verifier needs are inside (no external table).
/// No field may be a float.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarketDefinition {
    pub version: u32,
    pub params_version: u32,
    pub network: String,
    /// 16 bytes, lowercase hex.
    pub market_id: String,
    pub question: String,
    pub outcomes: Vec<OutcomeDefinition>,
    pub resolution_type: ResolutionType,
    pub resolution_params: ResolutionParams,
    pub birthday_height: u32,
    pub close_height: u32,
    pub resolve_height: u32,
    pub fee_bps: u32,
    pub min_bet_zat: u64,
    pub refund_fee_zat: u64,
    pub min_confirmations: u32,
    pub sweep_threshold_zat: u64,
    pub announce_timeout_blocks: u32,
    pub uivk: String,
    /// X25519 public key, base64url without padding.
    pub envelope_pk: String,
    pub treasury_address: String,
}

impl MarketDefinition {
    fn check(&self) -> Result<(), HashError> {
        let params_ok = matches!(
            (self.resolution_type, &self.resolution_params),
            (
                ResolutionType::PoolThreshold,
                ResolutionParams::PoolThreshold(_)
            ) | (
                ResolutionType::UpgradeActive,
                ResolutionParams::UpgradeActive(_)
            ) | (ResolutionType::Committee, ResolutionParams::Committee(_))
        );
        if !params_ok {
            return Err(HashError::Invalid(
                "resolution_params do not match resolution_type".into(),
            ));
        }
        if !is_lower_hex(&self.market_id, 16) {
            return Err(HashError::Invalid(
                "market_id must be 16 bytes of lowercase hex".into(),
            ));
        }
        Ok(())
    }
}

fn is_lower_hex(s: &str, bytes: usize) -> bool {
    s.len() == bytes * 2 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// `BLAKE2b-256(personal = "FrostedRules0001", JCS(market_definition))`.
pub fn rules_hash(def: &MarketDefinition) -> Result<[u8; 32], HashError> {
    def.check()?;
    hash_document(&PERSONAL_RULES, def)
}

/// Parses a published `market_definition` strictly (missing or unknown fields are errors) and
/// returns its `rules_hash`.
pub fn rules_hash_json(json: &str) -> Result<[u8; 32], HashError> {
    let def: MarketDefinition =
        serde_json::from_str(json).map_err(|e| HashError::Invalid(e.to_string()))?;
    rules_hash(&def)
}

// ---------------------------------------------------------------------------------------------
// Plan and plan_hash
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PlanKind {
    Standard,
    FollowUp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ItemKind {
    Win,
    Refund,
    Unclaimable,
    Sweep,
}

impl ItemKind {
    /// `SWEEP` and `UNCLAIMABLE` go to the public Treasury address.
    pub fn goes_to_treasury(self) -> bool {
        matches!(self, ItemKind::Sweep | ItemKind::Unclaimable)
    }
}

/// Winning `outcome_index`, or `"VOID"`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlanResult {
    Outcome(u32),
    Void,
}

impl Serialize for PlanResult {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            PlanResult::Outcome(i) => s.serialize_u32(*i),
            PlanResult::Void => s.serialize_str("VOID"),
        }
    }
}

impl<'de> Deserialize<'de> for PlanResult {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        match Value::deserialize(d)? {
            Value::String(s) if s == "VOID" => Ok(PlanResult::Void),
            Value::Number(n) => n
                .as_u64()
                .and_then(|v| u32::try_from(v).ok())
                .map(PlanResult::Outcome)
                .ok_or_else(|| serde::de::Error::custom("invalid outcome index")),
            _ => Err(serde::de::Error::custom(
                "result must be an outcome index or \"VOID\"",
            )),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanItem {
    pub note_ref: String,
    pub bet_fingerprint: Option<String>,
    pub kind: ItemKind,
    pub amount_zat: u64,
    pub receiver_commitment: Option<String>,
}

/// The public Payout Plan (JCS input of `plan_hash`). Contains no addresses. No float fields.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub market_id: String,
    pub plan_kind: PlanKind,
    pub plan_cutoff_height: u32,
    pub result: PlanResult,
    pub operator_fee_zat: u64,
    pub items: Vec<PlanItem>,
}

impl Plan {
    /// Sorts items by `note_ref` and validates item invariants.
    pub fn normalize(&mut self) -> Result<(), HashError> {
        self.items.sort_by(|a, b| a.note_ref.cmp(&b.note_ref));
        if self
            .items
            .windows(2)
            .any(|w| w[0].note_ref == w[1].note_ref)
        {
            return Err(HashError::Invalid("duplicate note_ref".into()));
        }
        for item in &self.items {
            if !is_lower_hex(&item.note_ref, 8) {
                return Err(HashError::Invalid(
                    "note_ref must be 8 bytes of lowercase hex".into(),
                ));
            }
            if item.kind.goes_to_treasury() && item.receiver_commitment.is_some() {
                return Err(HashError::Invalid(
                    "SWEEP and UNCLAIMABLE items must have receiver_commitment = null".into(),
                ));
            }
            if !item.kind.goes_to_treasury() && item.receiver_commitment.is_none() {
                return Err(HashError::Invalid(
                    "WIN and REFUND items need a receiver_commitment".into(),
                ));
            }
        }
        Ok(())
    }
}

/// `BLAKE2b-256(personal = "FrostedPlan00001", JCS(plan))`, with items sorted by `note_ref`.
pub fn plan_hash(plan: &Plan) -> Result<[u8; 32], HashError> {
    let mut plan = plan.clone();
    plan.normalize()?;
    hash_document(&PERSONAL_PLAN, &plan)
}

// ---------------------------------------------------------------------------------------------
// receiver_commitment and note_ref
// ---------------------------------------------------------------------------------------------

/// `hex(BLAKE2b-256(personal = "FrostedRcvr00001", orchard_receiver ‖ bet_salt))`.
pub fn receiver_commitment(receiver: &OrchardReceiver, bet_salt: &[u8; 8]) -> String {
    hex::encode(blake2b_256(&PERSONAL_RCVR, None, &[receiver, bet_salt]))
}

/// `hex(BLAKE2b-256(personal = "FrostedNoteRef01", txid ‖ action_index u32 LE)[0..8])`.
pub fn note_ref(txid: &[u8; 32], action_index: u32) -> String {
    let digest = blake2b_256(
        &PERSONAL_NOTE_REF,
        None,
        &[txid, &action_index.to_le_bytes()],
    );
    hex::encode(&digest[..8])
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use serde_json::json;

    pub(crate) fn sample_definition() -> MarketDefinition {
        MarketDefinition {
            version: 1,
            params_version: 1,
            network: "testnet".into(),
            market_id: "a1f3c0de9b7e4f21a0b1c2d3e4f50617".into(),
            question: "Apakah pool Ironwood ≥ 5.000.000 ZEC di blok H? 🧊".into(),
            outcomes: vec![
                OutcomeDefinition {
                    outcome_index: 0,
                    label: "YES".into(),
                    address: "utest1yes".into(),
                },
                OutcomeDefinition {
                    outcome_index: 1,
                    label: "NO".into(),
                    address: "utest1no".into(),
                },
            ],
            resolution_type: ResolutionType::PoolThreshold,
            resolution_params: ResolutionParams::PoolThreshold(PoolThresholdParams {
                pool: "ironwood".into(),
                comparator: "gte".into(),
                threshold_zat: 500_000_000_000_000,
            }),
            birthday_height: 4_519_000,
            close_height: 4_521_000,
            resolve_height: 4_521_100,
            fee_bps: 100,
            min_bet_zat: 1_000_000,
            refund_fee_zat: 5_000,
            min_confirmations: 3,
            sweep_threshold_zat: 5_000,
            announce_timeout_blocks: 100,
            uivk: "uivktest1example".into(),
            envelope_pk: "AAAA".into(),
            treasury_address: "utest1treasury".into(),
        }
    }

    pub(crate) fn sample_plan() -> Plan {
        Plan {
            market_id: "a1f3c0de9b7e4f21a0b1c2d3e4f50617".into(),
            plan_kind: PlanKind::Standard,
            plan_cutoff_height: 4_521_110,
            result: PlanResult::Outcome(0),
            operator_fee_zat: 12_400_000,
            items: vec![
                PlanItem {
                    note_ref: "1b2c3d4e5f607182".into(),
                    bet_fingerprint: None,
                    kind: ItemKind::Sweep,
                    amount_zat: 10_000,
                    receiver_commitment: None,
                },
                PlanItem {
                    note_ref: "0a1b2c3d4e5f6071".into(),
                    bet_fingerprint: Some("3fa1b2c4d5e6f708".into()),
                    kind: ItemKind::Win,
                    amount_zat: 79_714_285,
                    receiver_commitment: Some(receiver_commitment(&[1u8; 43], &[2u8; 8])),
                },
            ],
        }
    }

    // RFC 8785 §3.2.3 sorting example (all members, non-ASCII keys, surrogate pair).
    #[test]
    fn rfc8785_key_ordering() {
        let input = r#"{
          "\u20ac": "Euro Sign",
          "\r": "Carriage Return",
          "\ufb33": "Hebrew Letter Dalet With Dagesh",
          "1": "One",
          "\ud83d\ude00": "Emoji: Grinning Face",
          "\u0080": "Control",
          "\u00f6": "Latin Small Letter O With Diaeresis"
        }"#;
        let v: Value = serde_json::from_str(input).unwrap();
        let expected = "{\"\\r\":\"Carriage Return\",\"1\":\"One\",\"\u{80}\":\"Control\",\
                        \"\u{f6}\":\"Latin Small Letter O With Diaeresis\",\"\u{20ac}\":\"Euro Sign\",\
                        \"\u{1f600}\":\"Emoji: Grinning Face\",\"\u{fb33}\":\"Hebrew Letter Dalet With Dagesh\"}";
        assert_eq!(jcs(&v).unwrap(), expected);
    }

    // RFC 8785 §3.2.2.2 string example: "\u20ac$\u000F\u000aA'\u0042\u0022\u005c\\\"\/".
    #[test]
    fn rfc8785_string_escaping() {
        let v: Value =
            serde_json::from_str(r#""\u20ac$\u000F\u000aA'\u0042\u0022\u005c\\\"\/""#).unwrap();
        assert_eq!(jcs(&v).unwrap(), "\"€$\\u000f\\nA'B\\\"\\\\\\\\\\\"/\"");
    }

    // RFC 8785 §3.2.1 literals and nested structures (subset without floats).
    #[test]
    fn rfc8785_literals_and_nesting() {
        let v = json!({"b": [null, true, false, {"z": 1, "a": [2, -3]}], "a": {}, "c": []});
        assert_eq!(
            jcs(&v).unwrap(),
            r#"{"a":{},"b":[null,true,false,{"a":[2,-3],"z":1}],"c":[]}"#
        );
    }

    #[test]
    fn floats_and_unsafe_integers_rejected() {
        assert_eq!(jcs(&json!(1.5)), Err(HashError::Float));
        assert_eq!(
            jcs(&serde_json::from_str::<Value>("1.0").unwrap()),
            Err(HashError::Float)
        );
        assert!(matches!(
            jcs(&json!(1u64 << 53)),
            Err(HashError::UnsafeInteger(_))
        ));
        assert_eq!(jcs(&json!(MAX_SAFE_INTEGER)).unwrap(), "9007199254740991");
    }

    #[test]
    fn field_order_does_not_matter() {
        let a: Value = serde_json::from_str(r#"{"x":1,"y":{"b":2,"a":3}}"#).unwrap();
        let b: Value = serde_json::from_str(r#"{"y":{"a":3,"b":2},"x":1}"#).unwrap();
        assert_eq!(jcs(&a).unwrap(), jcs(&b).unwrap());
    }

    fn assert_no_floats(v: &Value) {
        match v {
            Value::Number(n) => assert!(n.is_u64() || n.is_i64(), "float in hashed schema: {n}"),
            Value::Array(a) => a.iter().for_each(assert_no_floats),
            Value::Object(m) => m.values().for_each(assert_no_floats),
            _ => {}
        }
    }

    #[test]
    fn hashed_schemas_have_no_floats() {
        assert_no_floats(&serde_json::to_value(sample_definition()).unwrap());
        let committee = MarketDefinition {
            resolution_type: ResolutionType::Committee,
            resolution_params: ResolutionParams::Committee(CommitteeParams {
                criteria: "c".into(),
                evidence_source: "e".into(),
                dispute_window_blocks: 144,
                void_after_blocks: 5_760,
            }),
            ..sample_definition()
        };
        assert_no_floats(&serde_json::to_value(committee).unwrap());
        assert_no_floats(&serde_json::to_value(sample_plan()).unwrap());
    }

    #[test]
    fn rules_hash_is_deterministic_and_sensitive() {
        let def = sample_definition();
        let h = rules_hash(&def).unwrap();
        let json = serde_json::to_string(&def).unwrap();
        assert_eq!(rules_hash_json(&json).unwrap(), h);
        // Reordered members give the same hash.
        let mut v: serde_json::Map<String, Value> = serde_json::from_str(&json).unwrap();
        let reordered: serde_json::Map<String, Value> = v.clone().into_iter().rev().collect();
        assert_eq!(
            rules_hash_json(&serde_json::to_string(&reordered).unwrap()).unwrap(),
            h
        );
        for changed in [
            MarketDefinition {
                envelope_pk: "BBBB".into(),
                ..def.clone()
            },
            MarketDefinition {
                treasury_address: "utest1other".into(),
                ..def.clone()
            },
            MarketDefinition {
                birthday_height: 1,
                ..def.clone()
            },
            MarketDefinition {
                sweep_threshold_zat: 6_000,
                ..def.clone()
            },
        ] {
            assert_ne!(rules_hash(&changed).unwrap(), h);
        }
        // Missing and unknown fields are errors.
        v.remove("treasury_address");
        assert!(rules_hash_json(&serde_json::to_string(&v).unwrap()).is_err());
        v.insert("treasury_address".into(), json!("utest1treasury"));
        v.insert("extra".into(), json!(1));
        assert!(rules_hash_json(&serde_json::to_string(&v).unwrap()).is_err());
    }

    #[test]
    fn mismatched_resolution_params_rejected() {
        let def = MarketDefinition {
            resolution_type: ResolutionType::Committee,
            ..sample_definition()
        };
        assert!(rules_hash(&def).is_err());
    }

    #[test]
    fn plan_hash_ignores_item_order_and_checks_commitments() {
        let plan = sample_plan();
        let mut reversed = plan.clone();
        reversed.items.reverse();
        assert_eq!(plan_hash(&plan).unwrap(), plan_hash(&reversed).unwrap());

        let mut bad = plan.clone();
        bad.items[0].receiver_commitment = Some("00".into());
        assert!(plan_hash(&bad).is_err());

        let json = serde_json::to_string(&plan).unwrap();
        assert!(json.contains("\"result\":0"));
        let void = Plan {
            result: PlanResult::Void,
            ..plan
        };
        assert!(
            serde_json::to_string(&void)
                .unwrap()
                .contains("\"result\":\"VOID\"")
        );
    }

    #[test]
    fn note_ref_and_receiver_commitment_shapes() {
        assert_eq!(note_ref(&[0u8; 32], 0).len(), 16);
        assert_ne!(note_ref(&[0u8; 32], 0), note_ref(&[0u8; 32], 1));
        assert_eq!(receiver_commitment(&[1u8; 43], &[2u8; 8]).len(), 64);
        assert_ne!(
            receiver_commitment(&[1u8; 43], &[2u8; 8]),
            receiver_commitment(&[1u8; 43], &[3u8; 8])
        );
    }
}

//! Browser-only Bet Builder. Secret material is returned to the caller, never transmitted.
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use zcash_glass::{address, envelope, hash, memo, params, thaw};

const MAX_MONEY: u64 = 2_100_000_000_000_000;
type Error = String;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildBet {
    pub market_definition: hash::MarketDefinition,
    pub outcome_index: u32,
    pub amount_zat: u64,
    pub payout_ua: String,
    pub k_root: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct BuiltBet {
    pub memo_text: String,
    pub zip321_uri: String,
    pub bet_fingerprint: String,
    pub env_esk: String,
    pub bet_salt: String,
}

fn hex_bytes<const N: usize>(s: &str) -> Result<[u8; N], Error> {
    if s.len() != N * 2 || !s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f')) {
        return Err("INVALID_INPUT: expected lowercase hex".into());
    }
    let mut bytes = [0; N];
    hex::decode_to_slice(s, &mut bytes).map_err(|_| "INVALID_INPUT".to_owned())?;
    Ok(bytes)
}

fn random<const N: usize>() -> Result<[u8; N], Error> {
    let mut bytes = [0; N];
    getrandom::fill(&mut bytes).map_err(|_| "RANDOMNESS_UNAVAILABLE".to_owned())?;
    Ok(bytes)
}

fn payment_uri(address: &str, amount: u64, memo: &str) -> String {
    format!(
        "zcash:{address}?amount={}.{:08}&memo={}",
        amount / 100_000_000,
        amount % 100_000_000,
        URL_SAFE_NO_PAD.encode(memo)
    )
}

pub fn build_with_randomness(
    input: BuildBet,
    randomness: envelope::SealRandomness,
    salt: [u8; 8],
) -> Result<BuiltBet, Error> {
    let market = &input.market_definition;
    let outcome_index = u8::try_from(input.outcome_index)
        .map_err(|_| "INVALID_INPUT: outcome index exceeds 255")?;
    hash::rules_hash(market).map_err(|e| e.to_string())?;
    if input.amount_zat > MAX_MONEY {
        return Err("INVALID_INPUT: amount exceeds MAX_MONEY".into());
    }
    if input.amount_zat < market.min_bet_zat {
        return Err("AMOUNT_BELOW_MIN".into());
    }
    let mut outcomes = market
        .outcomes
        .iter()
        .filter(|o| o.outcome_index == input.outcome_index);
    let outcome = outcomes.next().ok_or("INVALID_INPUT: unknown outcome")?;
    if outcomes.next().is_some() {
        return Err("INVALID_INPUT: duplicate outcome".into());
    }
    // A payment destination must be a same-network Orchard UA before entering the URI.
    address::parse_payout_ua(&outcome.address, &market.network).map_err(|e| e.to_string())?;
    let receiver =
        address::parse_payout_ua(&input.payout_ua, &market.network).map_err(|e| e.to_string())?;
    let market_id = hex_bytes(&market.market_id)?;
    let env_epk = envelope::envelope_public_key(&randomness.env_esk);
    let tag = input
        .k_root
        .as_deref()
        .map(|key| {
            hex_bytes(key)
                .map(|key| thaw::tag(&key, &market_id, outcome_index, input.amount_zat, &env_epk))
        })
        .transpose()?;
    let header = memo::Frst1Header::new(market_id, tag);
    let pk: [u8; 32] = URL_SAFE_NO_PAD
        .decode(&market.envelope_pk)
        .map_err(|_| "INVALID_INPUT: envelope_pk")?
        .try_into()
        .map_err(|_| "INVALID_INPUT: envelope_pk length")?;
    let sealed =
        envelope::seal(&pk, &header, &receiver, &salt, randomness).map_err(|e| e.to_string())?;
    let payload = sealed.into_payload(header);
    let memo_text = memo::encode_frst1(&payload).map_err(|e| e.to_string())?;
    Ok(BuiltBet {
        zip321_uri: payment_uri(&outcome.address, input.amount_zat, &memo_text),
        bet_fingerprint: memo::bet_fingerprint(&payload),
        env_esk: hex::encode(sealed.env_esk),
        bet_salt: hex::encode(salt),
        memo_text,
    })
}

fn to_js<T: Serialize>(value: &T) -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(value).map_err(|e| JsValue::from_str(&e.to_string()))
}
fn from_js<T: for<'de> Deserialize<'de>>(value: JsValue) -> Result<T, JsValue> {
    serde_wasm_bindgen::from_value(value)
        .map_err(|e| JsValue::from_str(&format!("INVALID_INPUT: {e}")))
}

#[wasm_bindgen]
pub fn build_bet(input: JsValue) -> Result<JsValue, JsValue> {
    let input = from_js(input)?;
    let result = (|| {
        build_with_randomness(
            input,
            envelope::SealRandomness {
                env_esk: random()?,
                env_nonce: random()?,
            },
            random()?,
        )
    })();
    to_js(&result.map_err(|e| JsValue::from_str(&e))?)
}

#[wasm_bindgen]
pub fn parse_payout_ua(ua: &str, network: &str) -> Result<JsValue, JsValue> {
    match address::parse_payout_ua(ua, network) {
        Ok(receiver) => to_js(&serde_json_result(true, Some(hex::encode(receiver)), None)),
        Err(e) => to_js(&serde_json_result(false, None, Some(e.to_string()))),
    }
}
#[derive(Serialize)]
struct ParsedAddress {
    ok: bool,
    receiver: Option<String>,
    error: Option<String>,
}
fn serde_json_result(ok: bool, receiver: Option<String>, error: Option<String>) -> ParsedAddress {
    ParsedAddress {
        ok,
        receiver,
        error,
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Persona {
    pub k_root: String,
    pub persona_id: String,
    pub frstp1_uri: String,
}
fn persona_with_key(registry: &str, key: [u8; 32]) -> Result<Persona, Error> {
    receipt_receiver(registry)?;
    let commitment = thaw::commitment(&key);
    Ok(Persona {
        k_root: hex::encode(key),
        persona_id: thaw::persona_id(&commitment),
        frstp1_uri: payment_uri(
            registry,
            params::MIN_ANNOUNCE_ZAT,
            &memo::encode_frstp1(&commitment),
        ),
    })
}
#[wasm_bindgen]
pub fn new_persona(registry_address: &str) -> Result<JsValue, JsValue> {
    let result = (|| persona_with_key(registry_address, random()?))();
    to_js(&result.map_err(|e| JsValue::from_str(&e))?)
}

#[derive(Deserialize)]
pub struct Receipt {
    pub payout_ua: String,
    pub bet_salt: String,
    pub env_esk: String,
}
fn receipt_receiver(ua: &str) -> Result<zcash_glass::OrchardReceiver, Error> {
    ["mainnet", "testnet", "regtest"]
        .iter()
        .find_map(|network| address::parse_payout_ua(ua, network).ok())
        .ok_or_else(|| "PAYOUT_ADDRESS_NO_ORCHARD".into())
}
fn receipt_commitment(receipt: &Receipt) -> Result<String, Error> {
    Ok(hash::receiver_commitment(
        &receipt_receiver(&receipt.payout_ua)?,
        &hex_bytes(&receipt.bet_salt)?,
    ))
}
#[wasm_bindgen]
pub fn receiver_commitment(receipt: JsValue) -> Result<String, JsValue> {
    receipt_commitment(&from_js(receipt)?).map_err(|e| JsValue::from_str(&e))
}
#[derive(Debug, Serialize, Deserialize)]
pub struct EnvelopeProof {
    pub note_ref: String,
    pub env_esk: String,
    pub receiver: String,
    pub bet_salt: String,
}
fn receipt_proof(receipt: &Receipt, note_ref: &str) -> Result<EnvelopeProof, Error> {
    hex_bytes::<8>(note_ref)?;
    hex_bytes::<32>(&receipt.env_esk)?;
    hex_bytes::<8>(&receipt.bet_salt)?;
    Ok(EnvelopeProof {
        note_ref: note_ref.into(),
        env_esk: receipt.env_esk.clone(),
        receiver: hex::encode(receipt_receiver(&receipt.payout_ua)?),
        bet_salt: receipt.bet_salt.clone(),
    })
}
#[wasm_bindgen]
pub fn envelope_proof(receipt: JsValue, note_ref: &str) -> Result<JsValue, JsValue> {
    to_js(&receipt_proof(&from_js(receipt)?, note_ref).map_err(|e| JsValue::from_str(&e))?)
}

#[cfg(test)]
mod tests;

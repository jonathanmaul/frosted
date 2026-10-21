//! M2.7: `scan` against a real regtest transaction (fixture from docs/SPIKES.md §M0.3).
#![cfg(feature = "zcash")]

use serde_json::Value;
use zcash_glass::scan::{ActivationHeights, MarketViewingKey, ParsedTx, ScanNetwork};

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/m03-escrow-funding.json")).unwrap()
}

fn network(fx: &Value) -> ScanNetwork {
    let mut heights = ActivationHeights::default();
    for (name, h) in fx["activation_heights"].as_object().unwrap() {
        heights.set(name, h.as_u64().unwrap() as u32);
    }
    ScanNetwork::new(fx["network"].as_str().unwrap(), heights).unwrap()
}

#[test]
fn decrypts_bet_to_outcome_address() {
    let fx = fixture();
    let net = network(&fx);
    let key = MarketViewingKey::from_uivk(&net, fx["uivk"].as_str().unwrap()).unwrap();
    let bytes = hex::decode(fx["tx_hex"].as_str().unwrap()).unwrap();
    let tx = ParsedTx::parse(&bytes, fx["height"].as_u64().unwrap() as u32, &net).unwrap();

    let mut txid = tx.txid();
    txid.reverse(); // RPC shows txids in reversed byte order
    assert_eq!(hex::encode(txid), fx["txid"].as_str().unwrap());

    let notes = tx.decrypt(&key);
    assert_eq!(notes.len(), 1, "exactly one action belongs to the escrow");
    let n = &notes[0];
    let exp = &fx["expected"];
    assert_eq!(n.value_zat, exp["action_value_zat"].as_u64().unwrap());
    assert_eq!(n.diversifier_index, exp["diversifier_index"].as_u64());
    assert_eq!(n.memo_text(), exp["memo_text"].as_str());
    assert_eq!(
        tx.has_transparent_input(),
        exp["has_transparent_input"].as_bool().unwrap()
    );
}

#[test]
fn unrelated_key_decrypts_nothing() {
    use orchard::keys::{FullViewingKey, Scope, SpendingKey};
    let fx = fixture();
    let net = network(&fx);
    let other = FullViewingKey::from(&SpendingKey::from_bytes([0x2a; 32]).unwrap());
    let key = MarketViewingKey::from_orchard_ivk(other.to_ivk(Scope::External));
    let bytes = hex::decode(fx["tx_hex"].as_str().unwrap()).unwrap();
    let tx = ParsedTx::parse(&bytes, fx["height"].as_u64().unwrap() as u32, &net).unwrap();
    assert!(tx.decrypt(&key).is_empty());
}

#[test]
fn detects_transparent_inputs() {
    // Z-only Guard input: the M0.1 shield tx spends transparent coinbase UTXOs.
    let shield: Value =
        serde_json::from_str(include_str!("fixtures/m01-shield-transparent-input.json")).unwrap();
    let net = network(&fixture());
    let bytes = hex::decode(shield["tx_hex"].as_str().unwrap()).unwrap();
    let tx = ParsedTx::parse(&bytes, shield["height"].as_u64().unwrap() as u32, &net).unwrap();
    assert!(tx.has_transparent_input());

    // And the shielded escrow funding tx does not.
    let fx = fixture();
    let bytes = hex::decode(fx["tx_hex"].as_str().unwrap()).unwrap();
    let tx = ParsedTx::parse(&bytes, fx["height"].as_u64().unwrap() as u32, &net).unwrap();
    assert!(!tx.has_transparent_input());
}

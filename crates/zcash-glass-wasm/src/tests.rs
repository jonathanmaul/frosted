use super::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::*;
use zcash_address::unified::{Address, Encoding, Receiver};
use zcash_protocol::consensus::NetworkType;
#[cfg(target_arch = "wasm32")]
wasm_bindgen_test_configure!(run_in_browser);

fn ua(raw: [u8; 43]) -> String {
    Address::try_from_items(vec![Receiver::Orchard(raw)])
        .unwrap()
        .encode(&NetworkType::Test)
}
fn input() -> BuildBet {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../../docs/test-vectors/hash.json")).unwrap();
    let mut market: hash::MarketDefinition =
        serde_json::from_value(fixture["market_definition"].clone()).unwrap();
    market.market_id = "a1".repeat(16);
    market.envelope_pk = URL_SAFE_NO_PAD.encode(envelope::envelope_public_key(&[0x11; 32]));
    market.outcomes[0].address = ua([0x44; 43]);
    BuildBet {
        market_definition: market,
        outcome_index: 0,
        amount_zat: 50_000_000,
        payout_ua: ua([0x44; 43]),
        k_root: Some("5a".repeat(32)),
    }
}
fn randomness() -> envelope::SealRandomness {
    envelope::SealRandomness {
        env_esk: [0x22; 32],
        env_nonce: [0x33; 24],
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn native_vector_zip321_and_receipt_roundtrip() {
    let built = build_with_randomness(input(), randomness(), [0x55; 8]).unwrap();
    let vector: serde_json::Value =
        serde_json::from_str(include_str!("../../../docs/test-vectors/envelope.json")).unwrap();
    let payload = memo::decode_frst1(&built.memo_text).unwrap();
    assert_eq!(
        hex::encode(payload.to_bytes()),
        vector["expected"]["payload"].as_str().unwrap()
    );
    assert_eq!(built.memo_text.len(), 237);
    assert_eq!(built.bet_fingerprint, memo::bet_fingerprint(&payload));
    assert_eq!(built.env_esk, "22".repeat(32));
    assert_eq!(built.bet_salt, "55".repeat(8));
    let payment = zip321::TransactionRequest::from_uri(&built.zip321_uri).unwrap();
    let payment = &payment.payments()[&0];
    assert_eq!(payment.recipient_address().to_string(), ua([0x44; 43]));
    assert_eq!(u64::from(payment.amount().unwrap()), 50_000_000);
    assert_eq!(
        &payment.memo().unwrap().as_array()[..237],
        built.memo_text.as_bytes()
    );
    let receipt = Receipt {
        payout_ua: ua([0x44; 43]),
        bet_salt: built.bet_salt,
        env_esk: built.env_esk,
    };
    assert_eq!(
        receipt_commitment(&receipt).unwrap(),
        hash::receiver_commitment(&[0x44; 43], &[0x55; 8])
    );
    let proof = receipt_proof(&receipt, "0123456789abcdef").unwrap();
    envelope::verify_proof(
        &envelope::EnvelopeProof {
            env_esk: hex_bytes(&proof.env_esk).unwrap(),
            receiver: hex_bytes(&proof.receiver).unwrap(),
            bet_salt: hex_bytes(&proof.bet_salt).unwrap(),
        },
        &payload,
        &envelope::envelope_public_key(&[0x11; 32]),
    )
    .unwrap();
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn untagged_and_validation() {
    let mut bet = input();
    bet.k_root = None;
    let built = build_with_randomness(bet, randomness(), [0x55; 8]).unwrap();
    assert!(
        !memo::decode_frst1(&built.memo_text)
            .unwrap()
            .header
            .has_thaw_tag()
    );
    let mut bet = input();
    bet.amount_zat = 1;
    assert_eq!(
        build_with_randomness(bet, randomness(), [0; 8]).unwrap_err(),
        "AMOUNT_BELOW_MIN"
    );
    let mut bet = input();
    bet.amount_zat = MAX_MONEY + 1;
    assert!(build_with_randomness(bet, randomness(), [0; 8]).is_err());
    let mut bet = input();
    bet.outcome_index = 99;
    assert!(build_with_randomness(bet, randomness(), [0; 8]).is_err());
    let mut bet = input();
    bet.market_definition.envelope_pk = URL_SAFE_NO_PAD.encode([0; 32]);
    assert!(build_with_randomness(bet, randomness(), [0; 8]).is_err());
    assert!(
        receipt_proof(
            &Receipt {
                payout_ua: ua([1; 43]),
                bet_salt: "00".repeat(8),
                env_esk: "00".repeat(32)
            },
            "invalid"
        )
        .is_err()
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn persona_registration() {
    let result = persona_with_key(&ua([7; 43]), [9; 32]).unwrap();
    assert_eq!(result.k_root, "09".repeat(32));
    let request = zip321::TransactionRequest::from_uri(&result.frstp1_uri).unwrap();
    let payment = &request.payments()[&0];
    assert_eq!(
        u64::from(payment.amount().unwrap()),
        params::MIN_ANNOUNCE_ZAT
    );
    let text = std::str::from_utf8(&payment.memo().unwrap().as_array()[..51]).unwrap();
    assert_eq!(
        memo::decode_frstp1(text).unwrap(),
        thaw::commitment(&[9; 32])
    );
    assert_eq!(
        result.persona_id,
        thaw::persona_id(&thaw::commitment(&[9; 32]))
    );
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen_test]
fn javascript_exports_use_webcrypto() {
    let value = input_for_js()
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .unwrap();
    let built: BuiltBet = serde_wasm_bindgen::from_value(build_bet(value).unwrap()).unwrap();
    assert_eq!(built.memo_text.len(), 237);
    let receipt = serde_json::json!({"payout_ua":ua([0x44;43]),"bet_salt":built.bet_salt,"env_esk":built.env_esk});
    let receipt_js = receipt
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .unwrap();
    let commitment = receiver_commitment(receipt_js.clone()).unwrap();
    assert_eq!(
        commitment,
        hash::receiver_commitment(&[0x44; 43], &hex_bytes(&built.bet_salt).unwrap())
    );
    let proof: EnvelopeProof =
        serde_wasm_bindgen::from_value(envelope_proof(receipt_js, "0123456789abcdef").unwrap())
            .unwrap();
    assert_eq!(proof.env_esk, built.env_esk);
    assert_eq!(proof.receiver, "44".repeat(43));
    let persona: Persona =
        serde_wasm_bindgen::from_value(new_persona(&ua([7; 43])).unwrap()).unwrap();
    assert_eq!(persona.k_root.len(), 64);
    let value = parse_payout_ua(&ua([7; 43]), "testnet").unwrap();
    let result: serde_json::Value = serde_wasm_bindgen::from_value(value).unwrap();
    assert_eq!(result["ok"], true);
}
#[cfg(target_arch = "wasm32")]
fn input_for_js() -> serde_json::Value {
    let input = input();
    serde_json::json!({"market_definition":input.market_definition,"outcome_index":input.outcome_index,"amount_zat":input.amount_zat,"payout_ua":input.payout_ua,"k_root":input.k_root})
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn persona_tag_is_thaw_tag_v2_over_the_bet_data() {
    let built = build_with_randomness(input(), randomness(), [0x55; 8]).unwrap();
    let payload = memo::decode_frst1(&built.memo_text).unwrap();
    assert!(payload.header.has_thaw_tag());
    let expected = thaw::tag(
        &[0x5a; 32],
        &[0xa1; 16],
        0,
        50_000_000,
        &envelope::envelope_public_key(&[0x22; 32]),
    );
    assert_eq!(payload.header.thaw_tag, expected);
    // The recomputation Thaw performs from public data accepts it, and rejects another key.
    let bet = thaw::TaggedBet {
        market_id: [0xa1; 16],
        outcome_index: 0,
        amount_zat: 50_000_000,
        env_epk: payload.env_epk,
        has_thaw_tag: true,
        thaw_tag: payload.header.thaw_tag,
    };
    assert!(thaw::matches(&[0x5a; 32], &bet));
    assert!(!thaw::matches(&[0x5b; 32], &bet));
}

//! Shared test vectors in `docs/test-vectors/*.json`, read by Rust tests now and by
//! `zcash-glass-wasm` tests in M3.
//!
//! Regenerate (only when a format change is approved by an ADR):
//! `FROSTED_WRITE_VECTORS=1 cargo test -p zcash-glass test_vectors`

use std::path::PathBuf;

use serde_json::{Value, json};

use crate::envelope::{self, EnvelopeProof, SealRandomness};
use crate::hash::{self, MarketDefinition, Plan};
use crate::memo::{self, Frst1Header};
use crate::payout::{self, EnvelopeResult, NoteInput, PlanInput};
use crate::status::BetStatus;
use crate::thaw;

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/test-vectors")
}

fn h(bytes: &[u8]) -> String {
    hex::encode(bytes)
}

fn arr<const N: usize>(v: &Value) -> [u8; N] {
    hex::decode(v.as_str().expect("hex string"))
        .expect("valid hex")
        .try_into()
        .expect("length")
}

/// Compares the computed vector with the file, or writes it when FROSTED_WRITE_VECTORS is set.
fn check(name: &str, computed: Value) {
    let path = dir().join(name);
    if std::env::var_os("FROSTED_WRITE_VECTORS").is_some() {
        std::fs::create_dir_all(dir()).unwrap();
        std::fs::write(
            &path,
            serde_json::to_string_pretty(&computed).unwrap() + "\n",
        )
        .unwrap();
        return;
    }
    let stored: Value = serde_json::from_str(
        &std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("missing {}: {e}", path.display())),
    )
    .unwrap();
    assert_eq!(stored, computed, "{name} does not match the implementation");
}

fn envelope_case() -> (Value, memo::Frst1Payload) {
    let envelope_sk = [0x11u8; 32];
    let envelope_pk = envelope::envelope_public_key(&envelope_sk);
    let market_id = [0xa1u8; 16];
    let k_root = [0x5au8; 32];
    let randomness = SealRandomness {
        env_esk: [0x22; 32],
        env_nonce: [0x33; 24],
    };
    let receiver = [0x44u8; 43];
    let bet_salt = [0x55u8; 8];
    let env_epk = envelope::envelope_public_key(&randomness.env_esk);
    let tag = thaw::tag(&k_root, &market_id, 0, 50_000_000, &env_epk);
    let header = Frst1Header::new(market_id, Some(tag));
    let sealed = envelope::seal(&envelope_pk, &header, &receiver, &bet_salt, randomness).unwrap();
    let payload = sealed.into_payload(header);
    let proof = EnvelopeProof {
        env_esk: sealed.env_esk,
        receiver,
        bet_salt,
    };
    envelope::verify_proof(&proof, &payload, &envelope_pk).unwrap();
    let v = json!({
        "envelope_sk": h(&envelope_sk),
        "envelope_pk": h(&envelope_pk),
        "market_id": h(&market_id),
        "thaw_tag": h(&tag),
        "env_esk": h(&randomness.env_esk),
        "env_nonce": h(&randomness.env_nonce),
        "receiver": h(&receiver),
        "bet_salt": h(&bet_salt),
        "expected": {
            "env_epk": h(&sealed.env_epk),
            "env_ciphertext": h(&sealed.env_ciphertext),
            "payload": h(&payload.to_bytes()),
            "proof_valid": true
        }
    });
    (v, payload)
}

#[test]
fn memo_vectors() {
    let (_, payload) = envelope_case();
    let untagged = memo::Frst1Payload {
        header: Frst1Header::new(payload.header.market_id, None),
        ..payload
    };
    let rules = [0x9fu8; 32];
    let commitment = thaw::commitment(&[0x5au8; 32]);
    let mut cases = vec![];
    for (name, p) in [("tagged", payload), ("untagged", untagged)] {
        let text = memo::encode_frst1(&p).unwrap();
        assert_eq!(memo::decode_frst1(&text).unwrap(), p);
        cases.push(json!({
            "name": name,
            "payload": h(&p.to_bytes()),
            "memo_text": text,
            "memo_len": text.len(),
            "bet_fingerprint": memo::bet_fingerprint(&p),
        }));
    }
    check(
        "memo.json",
        json!({
            "frst1": cases,
            "frstm1": { "rules_hash": h(&rules), "memo_text": memo::encode_frstm1(&rules) },
            "frstp1": { "persona_commitment": h(&commitment), "memo_text": memo::encode_frstp1(&commitment) },
        }),
    );
}

#[test]
fn envelope_vectors() {
    let (v, _) = envelope_case();
    // Re-derive from the stored inputs as an independent implementation would.
    let header = Frst1Header::new(arr(&v["market_id"]), Some(arr(&v["thaw_tag"])));
    let sealed = envelope::seal(
        &arr(&v["envelope_pk"]),
        &header,
        &arr(&v["receiver"]),
        &arr(&v["bet_salt"]),
        SealRandomness {
            env_esk: arr(&v["env_esk"]),
            env_nonce: arr(&v["env_nonce"]),
        },
    )
    .unwrap();
    let payload = sealed.into_payload(header);
    assert_eq!(
        envelope::open(&arr(&v["envelope_sk"]), &payload).unwrap(),
        (arr(&v["receiver"]), arr(&v["bet_salt"]))
    );
    check("envelope.json", v);
}

#[test]
fn thaw_vectors() {
    let k_root = [0x5au8; 32];
    let market_id = [0xa1u8; 16];
    let env_epk = [0x02u8; 32];
    let c = thaw::commitment(&k_root);
    check(
        "thaw.json",
        json!({
            "k_root": h(&k_root),
            "persona_commitment": h(&c),
            "persona_id": thaw::persona_id(&c),
            "tags": [
                { "market_id": h(&market_id), "outcome_index": 0, "amount_zat": 50_000_000u64,
                  "env_epk": h(&env_epk), "thaw_tag": h(&thaw::tag(&k_root, &market_id, 0, 50_000_000, &env_epk)) },
                { "market_id": h(&market_id), "outcome_index": 1, "amount_zat": 50_000_000u64,
                  "env_epk": h(&env_epk), "thaw_tag": h(&thaw::tag(&k_root, &market_id, 1, 50_000_000, &env_epk)) },
                { "market_id": h(&market_id), "outcome_index": 0, "amount_zat": 49_000_000u64,
                  "env_epk": h(&env_epk), "thaw_tag": h(&thaw::tag(&k_root, &market_id, 0, 49_000_000, &env_epk)) }
            ]
        }),
    );
}

#[test]
fn hash_vectors() {
    let def: MarketDefinition = hash::tests::sample_definition();
    let plan: Plan = hash::tests::sample_plan();
    let def_value = serde_json::to_value(&def).unwrap();
    let plan_value = serde_json::to_value(&plan).unwrap();
    let mut sorted_plan = plan.clone();
    sorted_plan.normalize().unwrap();
    check(
        "hash.json",
        json!({
            "jcs": [
                { "input": r#"{"b":[null,true,false,{"z":1,"a":[2,-3]}],"a":{},"c":[]}"#,
                  "expected": r#"{"a":{},"b":[null,true,false,{"a":[2,-3],"z":1}],"c":[]}"# },
                { "input": r#""\u20ac$\u000F\u000aA'\u0042\u0022\u005c\\\"\/""#,
                  "expected": "\"€$\\u000f\\nA'B\\\"\\\\\\\\\\\"/\"" }
            ],
            "market_definition": def_value,
            "market_definition_jcs": hash::jcs(&def_value).unwrap(),
            "rules_hash": h(&hash::rules_hash(&def).unwrap()),
            "plan": plan_value,
            "plan_jcs": hash::jcs(&serde_json::to_value(&sorted_plan).unwrap()).unwrap(),
            "plan_hash": h(&hash::plan_hash(&plan).unwrap()),
            "receiver_commitment": {
                "receiver": h(&[1u8; 43]), "bet_salt": h(&[2u8; 8]),
                "expected": hash::receiver_commitment(&[1u8; 43], &[2u8; 8])
            },
            "note_ref": {
                "txid": h(&[0x07u8; 32]), "action_index": 3,
                "expected": hash::note_ref(&[0x07u8; 32], 3)
            }
        }),
    );
}

#[test]
fn jcs_vectors_from_file_cases() {
    // The JCS cases are also checked by parsing the inputs, as a WASM implementation would.
    for (input, expected) in [
        (
            r#"{"b":[null,true,false,{"z":1,"a":[2,-3]}],"a":{},"c":[]}"#,
            r#"{"a":{},"b":[null,true,false,{"a":[2,-3],"z":1}],"c":[]}"#,
        ),
        (
            r#""\u20ac$\u000F\u000aA'\u0042\u0022\u005c\\\"\/""#,
            "\"€$\\u000f\\nA'B\\\"\\\\\\\\\\\"/\"",
        ),
    ] {
        let v: Value = serde_json::from_str(input).unwrap();
        assert_eq!(hash::jcs(&v).unwrap(), expected);
    }
}

fn payout_note(
    i: u16,
    status: BetStatus,
    outcome: Option<u32>,
    amount: u64,
    envelope: EnvelopeResult,
) -> NoteInput {
    let mut n = payout::tests::note(i, status, outcome, amount);
    n.envelope = envelope;
    n
}

#[test]
fn payout_vectors() {
    let mut notes = payout::tests::example_notes();
    notes.push(payout_note(
        30,
        BetStatus::Late,
        Some(0),
        20_000_000,
        payout::tests::opened(30),
    ));
    let mut announce = payout_note(
        31,
        BetStatus::InvalidMemo,
        Some(0),
        10_000,
        EnvelopeResult::None,
    );
    announce.is_announcement = true;
    notes.push(announce);
    notes.push(payout_note(
        32,
        BetStatus::UneconomicDust,
        None,
        1,
        EnvelopeResult::None,
    ));
    let input: PlanInput = payout::tests::input(crate::hash::PlanResult::Outcome(0), notes);
    let out = payout::plan(&input).unwrap();
    let notes_json: Vec<Value> = input
        .notes
        .iter()
        .map(|n| {
            json!({
                "note_ref": n.note_ref, "bet_fingerprint": n.bet_fingerprint,
                "outcome_index": n.outcome_index, "amount_zat": n.amount_zat,
                "block_height": n.block_height, "status": n.status,
                "is_announcement": n.is_announcement,
                "envelope": match n.envelope {
                    EnvelopeResult::Opened { receiver, bet_salt } =>
                        json!({ "receiver": h(&receiver), "bet_salt": h(&bet_salt) }),
                    EnvelopeResult::Unopenable => json!("UNOPENABLE"),
                    EnvelopeResult::None => Value::Null,
                }
            })
        })
        .collect();
    check(
        "payout.json",
        json!({
            "input": {
                "market_id": input.market_id, "plan_kind": "STANDARD",
                "plan_cutoff_height": input.plan_cutoff_height, "result": 0,
                "fee_bps": input.fee_bps, "refund_fee_zat": input.refund_fee_zat,
                "fee_model": { "marginal_fee_zat": input.fee_model.marginal_fee_zat,
                               "grace_actions": input.fee_model.grace_actions,
                               "max_actions_per_tx": input.fee_model.max_actions_per_tx },
                "notes": notes_json
            },
            "expected": {
                "plan": serde_json::to_value(&out.public).unwrap(),
                "plan_hash": h(&hash::plan_hash(&out.public).unwrap()),
                "tx_fee_zat": out.summary.tx_fee_zat,
                "treasury_out_zat": out.private.treasury_out_zat
            }
        }),
    );
}

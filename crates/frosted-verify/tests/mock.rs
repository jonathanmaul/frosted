//! The verifier against a fake node and a fake API. The honest scenario must PASS everywhere;
//! every manipulation below must FAIL with a diff. Chain scanning itself is covered live on regtest.

use std::collections::BTreeMap;

use anyhow::Result;
use frosted_verify::{
    Api, Command, Node, Note, ProofFile, Registration, Report, Scan, Status, checks,
};
use orchard::keys::{FullViewingKey, Scope, SpendingKey};
use serde_json::{Value, json};
use zcash_glass::{
    envelope::{self, SealRandomness},
    hash::{self, MarketDefinition, Plan, PlanKind, PlanResult},
    memo::{Frst1Header, Frst1Payload, bet_fingerprint},
    payout::{self, EnvelopeResult, FeeModel, NoteInput, PlanInput},
    scan::{ActivationHeights, ScanNetwork},
    status::BetStatus,
    thaw,
};
use zcash_keys::{address::UnifiedAddress, keys::UnifiedFullViewingKey};

const TIP: u32 = 220;
const MARKET: &str = "a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1";
const ENVELOPE_SK: [u8; 32] = [0x11; 32];
const K_ROOT: [u8; 32] = [0x5a; 32];

fn escrow() -> (String, Vec<String>) {
    let net = ScanNetwork::new("regtest", ActivationHeights::default()).unwrap();
    let fvk = FullViewingKey::from(&SpendingKey::from_bytes([0x44; 32]).unwrap());
    let addrs = (0u32..2)
        .map(|j| {
            UnifiedAddress::from_receivers(Some(fvk.address_at(j, Scope::External)), None)
                .unwrap()
                .encode(&net)
        })
        .collect();
    let uivk = UnifiedFullViewingKey::from_orchard_fvk(fvk)
        .unwrap()
        .to_unified_incoming_viewing_key()
        .encode(&net);
    (uivk, addrs)
}

fn definition() -> MarketDefinition {
    use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
    let (uivk, addrs) = escrow();
    serde_json::from_value(json!({"version":1,"params_version":1,"network":"regtest","market_id":MARKET,"question":"Pool target?",
        "outcomes":[{"outcome_index":0,"label":"YES","address":addrs[0]},{"outcome_index":1,"label":"NO","address":addrs[1]}],
        "resolution_type":"POOL_THRESHOLD","resolution_params":{"pool":"ironwood","comparator":"gte","threshold_zat":100_000_000u64},
        "birthday_height":100,"close_height":200,"resolve_height":210,"fee_bps":100,"min_bet_zat":1_000_000,"refund_fee_zat":5_000,
        "min_confirmations":3,"sweep_threshold_zat":5_000,"announce_timeout_blocks":100,"uivk":uivk,
        "envelope_pk":URL_SAFE_NO_PAD.encode(envelope::envelope_public_key(&ENVELOPE_SK)),"treasury_address":"utreasury"})).unwrap()
}

/// A sealed FRST1 payload with a real envelope, optionally Thaw-tagged for `K_ROOT`.
fn payload(outcome: u8, amount: u64, tagged: bool, seed: u8) -> (Frst1Payload, [u8; 32]) {
    let def = definition();
    let market: [u8; 16] = hex::decode(&def.market_id).unwrap().try_into().unwrap();
    let esk = [seed; 32];
    let epk = envelope::envelope_public_key(&esk);
    let tag = tagged.then(|| thaw::tag(&K_ROOT, &market, outcome, amount, &epk));
    let header = Frst1Header::new(market, tag);
    let pk = envelope::envelope_public_key(&ENVELOPE_SK);
    let sealed = envelope::seal(
        &pk,
        &header,
        &[0x77; 43],
        &[seed; 8],
        SealRandomness {
            env_esk: esk,
            env_nonce: [seed; 24],
        },
    )
    .unwrap();
    (sealed.into_payload(header), esk)
}

fn note(
    n: u8,
    outcome: Option<u32>,
    amount: u64,
    height: u32,
    status: BetStatus,
    p: Option<Frst1Payload>,
) -> Note {
    Note {
        note_ref: format!("{n:016x}"),
        outcome_index: outcome,
        amount_zat: amount,
        block_height: height,
        tx_index: 0,
        action_index: 0,
        status,
        is_announcement: false,
        bet_fingerprint: p.as_ref().map(bet_fingerprint),
        payload: p,
    }
}

fn scenario_notes() -> Vec<Note> {
    let mut announcement = note(1, Some(0), 10_000, 101, BetStatus::InvalidMemo, None);
    announcement.is_announcement = true;
    vec![
        announcement,
        note(
            2,
            Some(0),
            5_000_000,
            110,
            BetStatus::Pending,
            Some(payload(0, 5_000_000, true, 2).0),
        ), // YES, tagged
        note(
            3,
            Some(1),
            3_000_000,
            120,
            BetStatus::Pending,
            Some(payload(1, 3_000_000, true, 3).0),
        ), // NO, tagged
        note(
            4,
            Some(0),
            2_000_000,
            130,
            BetStatus::Pending,
            Some(payload(0, 2_000_000, false, 4).0),
        ),
        note(5, Some(0), 3_000, 140, BetStatus::UneconomicDust, None),
        note(
            6,
            Some(0),
            1_000_000,
            205,
            BetStatus::Late,
            Some(payload(0, 1_000_000, false, 6).0),
        ),
    ]
}

#[derive(Clone)]
struct FakeNode {
    tip: u32,
    scan: Scan,
    pool_value: u64,
    regs: Vec<Registration>,
}
impl FakeNode {
    fn honest() -> Self {
        let notes = scenario_notes();
        FakeNode {
            tip: TIP,
            scan: Scan {
                tip: TIP,
                announcement: Some((notes[0].note_ref.clone(), 101)),
                first_frstm1_hash: Some(hash::rules_hash(&definition()).unwrap()),
                notes,
            },
            pool_value: 200_000_000,
            regs: vec![Registration {
                commitment: thaw::commitment(&K_ROOT),
                height: 105,
            }],
        }
    }
}
impl Node for FakeNode {
    async fn network_name(&self) -> Result<String> {
        Ok("regtest".into())
    }
    async fn tip(&self) -> Result<u32> {
        Ok(self.tip)
    }
    async fn scan(&self, _: &MarketDefinition, _: &[u8; 32], upto: u32) -> Result<Scan> {
        let mut scan = self.scan.clone();
        scan.notes.retain(|n| n.block_height <= upto);
        Ok(scan)
    }
    async fn resolution_data(&self, _: u32) -> Result<(Value, Value)> {
        Ok((
            json!({"hash":"blockhash","valuePools":[{"id":"ironwood","chainValueZat":self.pool_value}]}),
            json!({"upgrades":{}}),
        ))
    }
    async fn registrations(&self, _: &str, _: &str, _: u32, _: u32) -> Result<Vec<Registration>> {
        Ok(self.regs.clone())
    }
}

#[derive(Clone)]
struct FakeApi {
    market: Value,
    odds: Value,
    resolution: Value,
    payouts: Vec<Value>,
    bets: Value,
    persona: Option<Value>,
    thaw_report: Option<Value>,
}
impl Api for FakeApi {
    async fn market(&self, _: &str) -> Result<Value> {
        Ok(self.market.clone())
    }
    async fn odds(&self, _: &str) -> Result<Value> {
        Ok(self.odds.clone())
    }
    async fn resolution(&self, _: &str) -> Result<Value> {
        Ok(self.resolution.clone())
    }
    async fn payouts(&self, _: &str) -> Result<Vec<Value>> {
        Ok(self.payouts.clone())
    }
    async fn market_ids(&self) -> Result<Vec<String>> {
        Ok(vec![MARKET.into()])
    }
    async fn indexer_height(&self) -> Result<u32> {
        Ok(TIP)
    }
    async fn bets(&self, _: &str) -> Result<Value> {
        Ok(self.bets.clone())
    }
    async fn persona(&self, _: &str) -> Result<Option<Value>> {
        Ok(self.persona.clone())
    }
    async fn thaw_report(&self, _: &str) -> Result<Option<Value>> {
        Ok(self.thaw_report.clone())
    }
    async fn registry(&self) -> Result<Option<Value>> {
        Ok(Some(
            json!({"uivk":"registry-uivk","address":"uregtest1registry","birthday_height":1}),
        ))
    }
}

impl FakeApi {
    /// What an honest coordinator and indexer would publish for the scenario.
    fn honest(node: &FakeNode) -> Self {
        let def = definition();
        let rules = hex::encode(hash::rules_hash(&def).unwrap());
        let notes = &node.scan.notes;
        let odds_bets: Vec<_> = notes
            .iter()
            .filter(|n| !n.is_announcement)
            .map(|n| zcash_glass::odds::OddsBet {
                outcome_index: n.outcome_index.unwrap_or(u32::MAX),
                amount_zat: n.amount_zat,
                status: n.status_at(TIP, 3),
            })
            .collect();
        let o = zcash_glass::odds::compute(2, &odds_bets);
        let odds = json!({"at_height":TIP,"pool_total_zat":o.pool_total_zat,"pending_bets":o.pending_bets,
            "outcomes":o.outcomes.iter().map(|x| json!({"outcome_index":x.outcome_index,"pool_zat":x.pool_zat,"bets":x.bets,"probability":x.probability})).collect::<Vec<_>>()});
        let resolution = json!({"market_id":MARKET,"status":"RESOLVED","winning_outcome_index":0,
            "evidence":{"source":"getblock","height":210,"block_hash":"blockhash","pool":"ironwood","chain_value_zat":200_000_000u64},"votes":[]});
        let inputs: Vec<NoteInput> = notes
            .iter()
            .map(|n| NoteInput {
                note_ref: n.note_ref.clone(),
                bet_fingerprint: n.bet_fingerprint.clone(),
                outcome_index: n.outcome_index,
                amount_zat: n.amount_zat,
                block_height: n.block_height,
                status: if n.is_announcement {
                    BetStatus::InvalidMemo
                } else {
                    n.status_at(TIP, 3)
                },
                is_announcement: n.is_announcement,
                envelope: if n.payload.is_some() {
                    EnvelopeResult::Opened {
                        receiver: [0x77; 43],
                        bet_salt: [9; 8],
                    }
                } else {
                    EnvelopeResult::None
                },
            })
            .collect();
        let out = payout::plan(&PlanInput {
            market_id: MARKET.into(),
            plan_kind: PlanKind::Standard,
            plan_cutoff_height: TIP,
            result: PlanResult::Outcome(0),
            fee_bps: 100,
            refund_fee_zat: 5_000,
            fee_model: FeeModel::default(),
            notes: inputs,
        })
        .unwrap();
        let s = &out.summary;
        let payout = json!({"plan_hash":hex::encode(hash::plan_hash(&out.public).unwrap()),"status":"CONFIRMED","plan":out.public,
            "summary":{"winners":s.winners,"refunds":s.refunds,"unclaimable":s.unclaimable,"swept":s.swept,"uneconomic_dust":s.uneconomic_dust,
                "operator_fee_zat":s.operator_fee_zat,"tx_fee_zat":s.tx_fee_zat,"distributed_zat":s.distributed_zat},
            "signing":{"sessions_done":1,"sessions_total":1,"threshold":2},"txs":[]});
        FakeApi {
            market: json!({"market_definition":def,"rules_hash":rules,"status":"SETTLED","announce_height":101}),
            odds,
            resolution,
            payouts: vec![payout],
            bets: json!({"bets":notes.iter().map(|n| json!({"note_ref":n.note_ref})).collect::<Vec<_>>()}),
            persona: None,
            thaw_report: None,
        }
    }
}

async fn all(api: &FakeApi, node: &FakeNode) -> Report {
    checks::run(api, node, MARKET, Command::All).await.unwrap()
}
fn status(r: &Report, id: &str) -> Status {
    r.checks
        .iter()
        .find(|c| c.id == id)
        .unwrap_or_else(|| panic!("no check {id}"))
        .status
}
fn failed(r: &Report) -> Vec<&str> {
    r.checks
        .iter()
        .filter(|c| c.status == Status::Fail)
        .map(|c| c.id.as_str())
        .collect()
}

#[tokio::test]
async fn honest_market_passes_every_check() {
    let node = FakeNode::honest();
    let r = all(&FakeApi::honest(&node), &node).await;
    assert!(r.ok, "{}", r.render());
    for id in [
        "rules_hash",
        "addresses",
        "announcement",
        "pools",
        "resolution",
        "plan_hash",
        "plan_items",
        "plan_result",
        "plan_summary",
    ] {
        assert_eq!(status(&r, id), Status::Pass, "{id}\n{}", r.render());
    }
    assert_eq!(r.api_height, Some(TIP));
}

#[tokio::test]
async fn pool_off_by_one_zat_fails_with_a_diff() {
    let node = FakeNode::honest();
    let mut api = FakeApi::honest(&node);
    api.odds["outcomes"][0]["pool_zat"] =
        json!(api.odds["outcomes"][0]["pool_zat"].as_u64().unwrap() + 1);
    let r = checks::run(&api, &node, MARKET, Command::Pools)
        .await
        .unwrap();
    assert!(!r.ok);
    let c = r.checks.iter().find(|c| c.id == "pools").unwrap();
    assert_eq!(c.status, Status::Fail);
    let d = c.diff.as_ref().unwrap();
    assert_eq!(
        d.api["outcomes"][0]["pool_zat"].as_u64().unwrap(),
        d.verifier["outcomes"][0]["pool_zat"].as_u64().unwrap() + 1
    );
    assert!(r.render().contains("[FAIL]"));
}

#[tokio::test]
async fn tampered_definition_fails_rules_hash_and_announcement() {
    let node = FakeNode::honest();
    let mut api = FakeApi::honest(&node);
    api.market["market_definition"]["question"] = json!("A different question?");
    // The API still serves the old hash, and the chain announced the old one.
    let r = checks::run(&api, &node, MARKET, Command::Market)
        .await
        .unwrap();
    assert!(failed(&r).contains(&"rules_hash"), "{}", r.render());
    let mut node2 = node.clone();
    node2.scan.announcement = None; // the announced hash no longer matches the tampered definition
    let r = checks::run(&api, &node2, MARKET, Command::Market)
        .await
        .unwrap();
    let c = r.checks.iter().find(|c| c.id == "announcement").unwrap();
    assert_eq!(c.status, Status::Fail);
    assert_ne!(
        c.diff.as_ref().unwrap().verifier,
        c.diff.as_ref().unwrap().api
    );
}

#[tokio::test]
async fn outcome_address_not_derived_from_uivk_fails() {
    let node = FakeNode::honest();
    let mut api = FakeApi::honest(&node);
    let (_, addrs) = escrow();
    api.market["market_definition"]["outcomes"][1]["address"] = json!(addrs[0]); // outcome 1 points at outcome 0
    let r = checks::run(&api, &node, MARKET, Command::Market)
        .await
        .unwrap();
    assert_eq!(status(&r, "addresses"), Status::Fail);
}

#[tokio::test]
async fn wrong_winner_fails_and_unbacked_winner_is_void() {
    let node = FakeNode::honest();
    let mut api = FakeApi::honest(&node);
    api.resolution["winning_outcome_index"] = json!(1);
    let r = checks::run(&api, &node, MARKET, Command::Resolution)
        .await
        .unwrap();
    assert_eq!(status(&r, "resolution"), Status::Fail);
    let d = r.checks[0].diff.as_ref().unwrap();
    assert_eq!(
        (
            d.verifier["winning_outcome_index"].as_u64(),
            d.api["winning_outcome_index"].as_u64()
        ),
        (Some(0), Some(1))
    );

    // The chain says NO wins but nobody bet on it: the honest resolution is VOID.
    let mut node = FakeNode::honest();
    node.pool_value = 1;
    node.scan.notes.retain(|n| n.outcome_index != Some(1));
    let mut api = FakeApi::honest(&node);
    api.resolution["status"] = json!("VOID");
    api.resolution["winning_outcome_index"] = Value::Null;
    api.resolution["evidence"]["chain_value_zat"] = json!(1);
    // winner is outcome 1 (below threshold) with zero counted money, so the verdict is VOID.
    let r = checks::run(&api, &node, MARKET, Command::Resolution)
        .await
        .unwrap();
    assert_eq!(status(&r, "resolution"), Status::Pass, "{}", r.render());
}

#[tokio::test]
async fn resolution_before_publication_is_skipped_and_committee_is_not_recomputed() {
    let node = FakeNode::honest();
    let mut api = FakeApi::honest(&node);
    api.resolution = Value::Null;
    let r = checks::run(&api, &node, MARKET, Command::Resolution)
        .await
        .unwrap();
    assert_eq!(status(&r, "resolution"), Status::Skip);
    assert!(r.ok);
}

#[tokio::test]
async fn altered_plan_item_fails_even_with_a_matching_plan_hash() {
    let node = FakeNode::honest();
    let mut api = FakeApi::honest(&node);
    let mut payout = api.payouts[0].clone();
    // The coordinator quietly pays a winner less and re-hashes the plan so plan_hash stays consistent.
    let item = payout["plan"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .position(|i| i["kind"] == "WIN")
        .unwrap();
    payout["plan"]["items"][item]["amount_zat"] = json!(
        payout["plan"]["items"][item]["amount_zat"]
            .as_u64()
            .unwrap()
            - 1
    );
    let plan: Plan = serde_json::from_value(payout["plan"].clone()).unwrap();
    payout["plan_hash"] = json!(hex::encode(hash::plan_hash(&plan).unwrap()));
    api.payouts = vec![payout];
    let r = all(&api, &node).await;
    assert_eq!(status(&r, "plan_hash"), Status::Pass);
    assert_eq!(status(&r, "plan_items"), Status::Fail, "{}", r.render());
    assert!(
        r.checks
            .iter()
            .find(|c| c.id == "plan_items")
            .unwrap()
            .diff
            .is_some()
    );
}

#[tokio::test]
async fn stale_plan_hash_and_swapped_kind_fail() {
    let node = FakeNode::honest();
    let mut api = FakeApi::honest(&node);
    api.payouts[0]["plan"]["operator_fee_zat"] = json!(1); // changed without re-hashing
    let r = all(&api, &node).await;
    assert!(failed(&r).contains(&"plan_hash"), "{}", r.render());

    let mut api = FakeApi::honest(&node);
    let mut payout = api.payouts[0].clone();
    let win = payout["plan"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .position(|i| i["kind"] == "WIN")
        .unwrap();
    payout["plan"]["items"][win]["kind"] = json!("REFUND");
    let plan: Plan = serde_json::from_value(payout["plan"].clone()).unwrap();
    payout["plan_hash"] = json!(hex::encode(hash::plan_hash(&plan).unwrap()));
    api.payouts = vec![payout];
    assert!(failed(&all(&api, &node).await).contains(&"plan_items"));
}

#[tokio::test]
async fn unclaimable_items_are_reported_separately_with_an_envelope_proof_hint() {
    let node = FakeNode::honest();
    let mut api = FakeApi::honest(&node);
    let mut payout = api.payouts[0].clone();
    let win = payout["plan"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .position(|i| i["kind"] == "WIN")
        .unwrap();
    // Declare the winner unclaimable (no receiver commitment): this is the lie an envelope proof disproves.
    payout["plan"]["items"][win]["kind"] = json!("UNCLAIMABLE");
    payout["plan"]["items"][win]["receiver_commitment"] = Value::Null;
    let plan: Plan = serde_json::from_value(payout["plan"].clone()).unwrap();
    payout["plan_hash"] = json!(hex::encode(hash::plan_hash(&plan).unwrap()));
    api.payouts = vec![payout];
    let r = all(&api, &node).await;
    let c = r.checks.iter().find(|c| c.id == "unclaimable").unwrap();
    assert_eq!(c.status, Status::Info);
    assert!(c.detail.contains("envelope-proof"));
}

fn proof_for(seed: u8, note_ref: &str) -> ProofFile {
    ProofFile {
        note_ref: note_ref.into(),
        env_esk: hex::encode([seed; 32]),
        receiver: hex::encode([0x77; 43]),
        bet_salt: hex::encode([seed; 8]),
    }
}

#[tokio::test]
async fn envelope_proof_passes_when_valid_and_fails_when_forged() {
    let node = FakeNode::honest();
    let mut api = FakeApi::honest(&node);
    let target = node.scan.notes[1].note_ref.clone(); // seeded with 2
    let mut payout = api.payouts[0].clone();
    let pos = payout["plan"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .position(|i| i["note_ref"] == target)
        .unwrap();
    payout["plan"]["items"][pos]["kind"] = json!("UNCLAIMABLE");
    api.payouts = vec![payout];

    let r = checks::envelope_proof(&api, &node, &proof_for(2, &target), None)
        .await
        .unwrap();
    assert!(r.ok, "{}", r.render());
    assert!(
        r.checks[0]
            .detail
            .contains("UNCLAIMABLE label for this note is wrong")
    );

    let forged = checks::envelope_proof(&api, &node, &proof_for(9, &target), None)
        .await
        .unwrap();
    assert!(!forged.ok);
    let mut wrong_receiver = proof_for(2, &target);
    wrong_receiver.receiver = hex::encode([0x78; 43]);
    assert!(
        !checks::envelope_proof(&api, &node, &wrong_receiver, None)
            .await
            .unwrap()
            .ok
    );
    assert!(
        checks::envelope_proof(&api, &node, &proof_for(2, "ffffffffffffffff"), None)
            .await
            .is_err(),
        "unknown note_ref"
    );
    let unknown =
        checks::envelope_proof(&api, &node, &proof_for(2, "ffffffffffffffff"), Some(MARKET))
            .await
            .unwrap();
    assert!(!unknown.ok);
}

fn honest_thaw_report(api: &FakeApi, node: &FakeNode) -> thaw::ThawReport {
    use zcash_glass::settle::{self, PlanItems, ResolutionView};
    let plan: Plan = serde_json::from_value(api.payouts[0]["plan"].clone()).unwrap();
    let items = PlanItems::from_plans([("CONFIRMED", &plan)]);
    let view = Some(ResolutionView {
        status: "RESOLVED",
        winning_outcome_index: Some(0),
    });
    let market: [u8; 16] = hex::decode(MARKET).unwrap().try_into().unwrap();
    let (mut bets, mut results) = (Vec::new(), BTreeMap::new());
    for n in node
        .scan
        .notes
        .iter()
        .filter(|n| n.payload.is_some_and(|p| p.header.has_thaw_tag()))
    {
        let p = n.payload.unwrap();
        let status = serde_json::to_value(n.status_at(TIP, 3)).unwrap();
        results.insert(
            n.note_ref.clone(),
            settle::settlement(
                status.as_str().unwrap(),
                n.outcome_index,
                &n.note_ref,
                view,
                &items,
            ),
        );
        bets.push(thaw::ReportBet {
            note_ref: n.note_ref.clone(),
            bet: thaw::TaggedBet {
                market_id: market,
                outcome_index: n.outcome_index.unwrap() as u8,
                amount_zat: n.amount_zat,
                env_epk: p.env_epk,
                has_thaw_tag: true,
                thaw_tag: p.header.thaw_tag,
            },
            block_height: n.block_height,
            tx_index: n.tx_index,
            action_index: n.action_index,
        });
    }
    thaw::report(&K_ROOT, "orion", 105, &bets, &results)
}

fn thawed_api(node: &FakeNode) -> (FakeApi, thaw::ThawReport) {
    let mut api = FakeApi::honest(node);
    let id = thaw::persona_id(&thaw::commitment(&K_ROOT));
    api.persona =
        Some(json!({"persona_id":id,"registered_height":105,"status":"THAWED","alias":"orion"}));
    // The report an honest API publishes: the same pure function over the same public facts.
    let report = honest_thaw_report(&api, node);
    api.thaw_report = Some(serde_json::to_value(&report).unwrap());
    (api, report)
}

#[tokio::test]
async fn thaw_report_is_identical_to_the_api_and_a_changed_report_fails() {
    let node = FakeNode::honest();
    let (api, report) = thawed_api(&node);
    assert_eq!(
        report.bets.len(),
        2,
        "two tagged bets, the untagged ones are not this Persona's"
    );
    let out = checks::thaw(&api, &node, &K_ROOT, None).await.unwrap();
    assert!(out.checks.ok, "{}", out.checks.render());
    assert_eq!(out.thaw, report);
    assert_eq!(out.thaw.stats.settled, 2);
    assert_eq!(
        (out.thaw.stats.won, out.thaw.bets.len()),
        (1, 2),
        "wins and losses both listed"
    );

    let mut tampered = api.clone();
    tampered.thaw_report.as_mut().unwrap()["stats"]["won"] = json!(2);
    let out = checks::thaw(&tampered, &node, &K_ROOT, None).await.unwrap();
    assert!(!out.checks.ok);
    assert_eq!(status(&out.checks, "thaw_report"), Status::Fail);

    let mut early = api.clone();
    early.persona.as_mut().unwrap()["registered_height"] = json!(1);
    assert_eq!(
        status(
            &checks::thaw(&early, &node, &K_ROOT, None)
                .await
                .unwrap()
                .checks,
            "registered"
        ),
        Status::Fail
    );
}

#[tokio::test]
async fn thaw_for_an_unregistered_or_unthawed_persona() {
    let node = FakeNode::honest();
    let mut api = FakeApi::honest(&node);
    let out = checks::thaw(&api, &node, &K_ROOT, Some("orion"))
        .await
        .unwrap();
    assert!(out.checks.ok);
    assert_eq!(status(&out.checks, "thaw_report"), Status::Info);
    assert_eq!(out.thaw.alias, "orion");
    api.persona = None;
    let mut empty = node.clone();
    empty.regs.clear();
    let out = checks::thaw(&api, &empty, &K_ROOT, None).await.unwrap();
    assert_eq!(status(&out.checks, "registered"), Status::Fail);
}

#[tokio::test]
async fn a_node_behind_the_api_is_an_error_not_a_pass() {
    let mut node = FakeNode::honest();
    let api = FakeApi::honest(&node);
    node.tip = TIP - 5;
    let err = checks::run(&api, &node, MARKET, Command::Pools)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("sync"), "{err}");
}

#[test]
fn json_round_trip_keeps_diff() {
    let mut r = Report::new("m", "all", 1);
    r.compare("x", "x", &1, &2, "");
    let again: Report = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
    assert_eq!(again, r);
    assert!(!again.ok);
    let _: BTreeMap<String, Value> = BTreeMap::new();
}

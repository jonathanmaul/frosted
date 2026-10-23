//! The checks. Each recomputes a public claim from the verifier's own node with `zcash-glass`
//! and compares it with what the API says. Nothing here duplicates protocol logic.

use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use zcash_glass::{
    address,
    envelope::{self, EnvelopeProof},
    hash::{self, ItemKind, MarketDefinition, Plan, PlanKind, PlanResult, ResolutionParams},
    odds::{self, OddsBet},
    payout::{self, EnvelopeResult, FeeModel, NoteInput, PlanInput},
    resolution,
    scan::{ActivationHeights, MarketViewingKey, ScanNetwork},
    settle::{self, PlanItems, ResolutionView},
    status::BetStatus,
    thaw::{self, ReportBet, TaggedBet, ThawReport},
};

use crate::{
    report::{Diff, Report},
    source::{Api, Node, Scan},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Market,
    Pools,
    Resolution,
    All,
}

impl Command {
    pub fn name(self) -> &'static str {
        match self {
            Command::Market => "market",
            Command::Pools => "pools",
            Command::Resolution => "resolution",
            Command::All => "all",
        }
    }
}

/// A market under check: its definition as the API serves it, plus the verifier's own scan.
struct Session<'a, A: Api, N: Node> {
    api: &'a A,
    node: &'a N,
    id: String,
    api_market: Value,
    def: MarketDefinition,
    rules_hash: [u8; 32],
    tip: u32,
    scan: Option<Scan>,
    /// Result of the resolution check when it could be recomputed, else the API's own.
    result: Option<PlanResult>,
    report: Report,
}

/// Runs `command` against `market_id`.
pub async fn run<A: Api, N: Node>(
    api: &A,
    node: &N,
    market_id: &str,
    command: Command,
) -> Result<Report> {
    let tip = node.tip().await?;
    let mut report = Report::new(market_id, command.name(), tip);
    let api_market = api.market(market_id).await?;
    let def: MarketDefinition =
        match serde_json::from_value(api_market["market_definition"].clone()) {
            Ok(def) => def,
            Err(e) => {
                report.fail(
                    "definition",
                    "market_definition is well formed",
                    e.to_string(),
                    None,
                );
                return Ok(report);
            }
        };
    let rules_hash = match hash::rules_hash(&def) {
        Ok(h) => h,
        Err(e) => {
            report.fail(
                "definition",
                "market_definition is well formed",
                e.to_string(),
                None,
            );
            return Ok(report);
        }
    };
    let mut s = Session {
        api,
        node,
        id: market_id.into(),
        api_market,
        def,
        rules_hash,
        tip,
        scan: None,
        result: None,
        report,
    };
    match command {
        Command::Market => s.market().await?,
        Command::Pools => s.pools().await?,
        Command::Resolution => s.resolution().await?,
        Command::All => {
            s.market().await?;
            s.pools().await?;
            s.resolution().await?;
            s.plan().await?;
        }
    }
    Ok(s.report)
}

impl<A: Api, N: Node> Session<'_, A, N> {
    async fn scan(&mut self) -> Result<&Scan> {
        if self.scan.is_none() {
            self.scan = Some(
                self.node
                    .scan(&self.def, &self.rules_hash, self.tip)
                    .await?,
            );
        }
        Ok(self.scan.as_ref().expect("scan set above"))
    }

    fn need_height(&self, height: u32, what: &str) -> Result<()> {
        if self.tip < height {
            bail!(
                "your node is at block {} but {what} needs block {height}; wait for it to sync",
                self.tip
            );
        }
        Ok(())
    }

    // ---------------------------------------------------------------- market

    async fn market(&mut self) -> Result<()> {
        let id = self.id.clone();
        let api_hash = self.api_market["rules_hash"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        self.report.compare(
            "rules_hash",
            "rules_hash = BLAKE2b-256(JCS(market_definition))",
            &hex::encode(self.rules_hash),
            &api_hash,
            hex::encode(self.rules_hash),
        );
        self.report.compare(
            "market_id",
            "market_id matches the definition",
            &self.def.market_id.clone(),
            &id,
            id.clone(),
        );

        // Every Outcome Address must derive from the published UIVK (US-01).
        let net = ScanNetwork::new(&self.def.network, ActivationHeights::default())?;
        match MarketViewingKey::from_uivk(&net, &self.def.uivk) {
            Err(e) => self.report.fail(
                "addresses",
                "Outcome Addresses derive from the UIVK",
                format!("cannot use the published UIVK: {e}"),
                None,
            ),
            Ok(key) => {
                let mut bad = Vec::new();
                for o in &self.def.outcomes {
                    let derived = key.receiver_at(o.outcome_index);
                    match address::parse_payout_ua(&o.address, &self.def.network) {
                        Ok(r) if r == derived => {}
                        _ => bad
                            .push(json!({"outcome_index": o.outcome_index, "address": o.address})),
                    }
                }
                if bad.is_empty() {
                    self.report.pass(
                        "addresses",
                        "Outcome Addresses derive from the UIVK",
                        format!("{} outcome addresses match", self.def.outcomes.len()),
                    );
                } else {
                    self.report.fail(
                        "addresses",
                        "Outcome Addresses derive from the UIVK",
                        "These addresses do not derive from the published UIVK.",
                        Some(Diff {
                            verifier: json!("derived from UIVK"),
                            api: json!(bad),
                        }),
                    );
                }
            }
        }

        // The announcement on this node pins the same rules_hash (ADR-0005).
        let api_announce = self.api_market["announce_height"]
            .as_u64()
            .map(|h| h as u32);
        let expected = hex::encode(self.rules_hash);
        let status = self.api_market["status"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let scan = self.scan().await?.clone();
        match (&scan.announcement, scan.first_frstm1_hash) {
            (Some((note_ref, height)), _) => {
                self.report.pass(
                    "announcement",
                    "On-chain announcement carries this rules_hash",
                    format!("note {note_ref} at block {height}"),
                );
                self.report.compare(
                    "announce_height",
                    "API announce_height matches the chain",
                    &Some(*height),
                    &api_announce,
                    format!("block {height}"),
                );
            }
            (None, Some(onchain)) => self.report.fail(
                "announcement",
                "On-chain announcement carries this rules_hash",
                "The first FRSTM1 announcement on your node pins a different rules_hash.",
                Some(Diff {
                    verifier: json!(hex::encode(onchain)),
                    api: json!(expected),
                }),
            ),
            (None, None) if api_announce.is_none() && status == "ANNOUNCING" => {
                self.report.info(
                    "announcement",
                    "On-chain announcement carries this rules_hash",
                    "Not announced yet; the market is still ANNOUNCING.",
                );
            }
            (None, None) => self.report.fail(
                "announcement",
                "On-chain announcement carries this rules_hash",
                "No FRSTM1 announcement for this market was found on your node.",
                None,
            ),
        }
        Ok(())
    }

    // ----------------------------------------------------------------- pools

    async fn pools(&mut self) -> Result<()> {
        let odds_api = self.api.odds(&self.id).await?;
        let at = odds_api["at_height"]
            .as_u64()
            .context("odds: no at_height")? as u32;
        self.report.api_height = Some(at);
        self.need_height(at, "the API's odds")?;
        let min_conf = self.def.min_confirmations;
        let count = self.def.outcomes.len() as u32;
        let scan = self.scan().await?;
        let bets: Vec<OddsBet> = scan
            .notes
            .iter()
            .filter(|n| !n.is_announcement && n.block_height <= at)
            .map(|n| OddsBet {
                outcome_index: n.outcome_index.unwrap_or(u32::MAX),
                amount_zat: n.amount_zat,
                status: n.status_at(at, min_conf),
            })
            .collect();
        let mine = odds::compute(count, &bets);
        let verifier = json!({
            "pool_total_zat": mine.pool_total_zat,
            "pending_bets": mine.pending_bets,
            "outcomes": mine.outcomes.iter().map(|o| json!({"outcome_index": o.outcome_index, "pool_zat": o.pool_zat, "bets": o.bets})).collect::<Vec<_>>(),
        });
        let api = json!({
            "pool_total_zat": odds_api["pool_total_zat"],
            "pending_bets": odds_api["pending_bets"],
            "outcomes": odds_api["outcomes"].as_array().map(|a| a.iter().map(|o| json!({"outcome_index": o["outcome_index"], "pool_zat": o["pool_zat"], "bets": o["bets"]})).collect::<Vec<_>>()).unwrap_or_default(),
        });
        let summary = mine
            .outcomes
            .iter()
            .map(|o| format!("{}: {} zat ({} bets)", o.outcome_index, o.pool_zat, o.bets))
            .collect::<Vec<_>>()
            .join(", ");
        self.report.compare(
            "pools",
            "Pools per outcome match the chain",
            &verifier,
            &api,
            format!(
                "at block {at}: {summary}; total {} zat",
                mine.pool_total_zat
            ),
        );
        Ok(())
    }

    // ------------------------------------------------------------ resolution

    async fn resolution(&mut self) -> Result<()> {
        const NAME: &str = "Chain-Verified resolution matches the chain";
        let published = self.api.resolution(&self.id).await?;
        if published.is_null() {
            self.report
                .skip("resolution", NAME, "No resolution has been published yet.");
            return Ok(());
        }
        let api_result = result_of(&published);
        if matches!(self.def.resolution_params, ResolutionParams::Committee(_)) {
            self.result = api_result;
            self.report.info("resolution", NAME, "Committee market: the outcome is a human decision, not recomputable from the chain. Votes and evidence are public in the API.");
            return Ok(());
        }
        self.need_height(self.def.resolve_height, "the resolution")?;
        let (block, info) = self.node.resolution_data(self.def.resolve_height).await?;
        let mut body =
            resolution::resolve(&self.def, &block, &info).map_err(|e| anyhow::anyhow!("{e}"))?;
        let winner = body["winning_outcome_index"].as_u64().map(|w| w as u32);
        let (min_conf, tip) = (self.def.min_confirmations, self.tip);
        let scan = self.scan().await?;
        let backed: u64 = scan
            .notes
            .iter()
            .filter(|n| {
                !n.is_announcement
                    && n.outcome_index == winner
                    && n.status_at(tip, min_conf) == BetStatus::Counted
            })
            .map(|n| n.amount_zat)
            .sum();
        resolution::void_if_unbacked(&mut body, backed);
        self.result = result_of(&body);

        let core = |v: &Value, upgrade: Option<&str>| {
            let e = &v["evidence"];
            let activation = upgrade.and_then(|u| {
                e["reported_upgrades"]
                    .as_array()?
                    .iter()
                    .find(|x| x["name"] == u)?["activation_height"]
                    .as_u64()
            });
            json!({"status": v["status"], "winning_outcome_index": v["winning_outcome_index"], "source": e["source"], "height": e["height"],
                "block_hash": e["block_hash"], "pool": e["pool"], "chain_value_zat": e["chain_value_zat"], "upgrade_activation_height": activation})
        };
        let upgrade = match &self.def.resolution_params {
            ResolutionParams::UpgradeActive(p) => Some(p.upgrade_name.clone()),
            _ => None,
        };
        let (mine, theirs) = (
            core(&body, upgrade.as_deref()),
            core(&published, upgrade.as_deref()),
        );
        let detail = format!(
            "{} (winning outcome {})",
            body["status"].as_str().unwrap_or_default(),
            body["winning_outcome_index"]
        );
        self.report
            .compare("resolution", NAME, &mine, &theirs, detail);

        // ADR-0016: a node that lists other upgrades than the coordinator's did is worth a look.
        if let Ok(mine) = resolution::reported_upgrades(&info) {
            let theirs: Vec<(String, u64)> = published["evidence"]["reported_upgrades"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|u| {
                            Some((
                                u["name"].as_str()?.to_owned(),
                                u["activation_height"].as_u64()?,
                            ))
                        })
                        .collect()
                })
                .unwrap_or_default();
            let (mut a, mut b) = (mine, theirs);
            a.sort();
            b.sort();
            if !b.is_empty() && a != b {
                self.report.info("upgrades", "Node upgrade list matches the evidence", "Your node lists different upgrades than the resolution evidence (ADR-0016). Check that both nodes run compatible versions.");
            }
        }
        Ok(())
    }

    // ------------------------------------------------------------------ plan

    async fn plan(&mut self) -> Result<()> {
        let plans = self.api.payouts(&self.id).await?;
        if plans.is_empty() {
            self.report.skip(
                "plan",
                "Payout plan matches the public bets",
                "No payout plan has been published yet.",
            );
            return Ok(());
        }
        let total = plans.len();
        let mut previous_cutoff: Option<u32> = None;
        for (i, published) in plans.iter().enumerate() {
            let suffix = if total > 1 {
                Some((i + 1, total))
            } else {
                None
            };
            let cutoff = self.plan_one(published, suffix, previous_cutoff).await?;
            // The coordinator starts a follow-up after the highest confirmed cutoff.
            if published["status"] == "CONFIRMED" {
                previous_cutoff = previous_cutoff.max(cutoff);
            }
        }
        Ok(())
    }

    /// Checks one plan. Returns its cutoff when it parsed.
    async fn plan_one(
        &mut self,
        published: &Value,
        suffix: Option<(usize, usize)>,
        previous_cutoff: Option<u32>,
    ) -> Result<Option<u32>> {
        let label = |s: &str| match suffix {
            Some((n, m)) => format!("{s} (plan {n} of {m})"),
            None => s.to_owned(),
        };
        let key = |s: &str| match suffix {
            Some((n, _)) => format!("{s}#{n}"),
            None => s.to_owned(),
        };
        let name = label("Payout plan matches the public bets");
        let plan: Plan = match serde_json::from_value(published["plan"].clone()) {
            Ok(p) => p,
            Err(e) => {
                self.report.fail(
                    &key("plan_hash"),
                    &label("plan_hash = BLAKE2b-256(JCS(plan))"),
                    format!("plan is malformed: {e}"),
                    None,
                );
                return Ok(None);
            }
        };
        let actual = hex::encode(hash::plan_hash(&plan)?);
        self.report.compare(
            &key("plan_hash"),
            &label("plan_hash = BLAKE2b-256(JCS(plan))"),
            &actual,
            &published["plan_hash"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            actual.clone(),
        );
        self.report.compare(
            &key("plan_market"),
            &label("Plan belongs to this market"),
            &plan.market_id,
            &self.id.clone(),
            self.id.clone(),
        );

        let unclaimable: Vec<&str> = plan
            .items
            .iter()
            .filter(|i| i.kind == ItemKind::Unclaimable)
            .map(|i| i.note_ref.as_str())
            .collect();
        if !unclaimable.is_empty() {
            self.report.info(&key("unclaimable"), &label("Items labelled UNCLAIMABLE"), format!(
                "{} item(s): {}. The envelope cannot be opened publicly. If one is yours, export an Envelope Proof from My Bets and run `frosted-verify envelope-proof <file>`.",
                unclaimable.len(), unclaimable.join(", ")));
        }

        let cutoff = plan.plan_cutoff_height;
        self.need_height(cutoff, "the plan cutoff")?;
        // A STANDARD plan pays the resolved result; a FOLLOW_UP refunds what arrived after the last confirmed cutoff.
        let (input_result, lower) = match plan.plan_kind {
            PlanKind::Standard => (self.result.unwrap_or(PlanResult::Void), None),
            PlanKind::FollowUp => match previous_cutoff {
                Some(c) => (PlanResult::Void, Some(c)),
                None => {
                    self.report.fail(
                        &key("plan_items"),
                        &name,
                        "A FOLLOW_UP plan appears without an earlier confirmed plan.",
                        None,
                    );
                    return Ok(Some(cutoff));
                }
            },
        };
        let declared: BTreeMap<&str, &hash::PlanItem> = plan
            .items
            .iter()
            .map(|i| (i.note_ref.as_str(), i))
            .collect();
        let min_conf = self.def.min_confirmations;
        let (fee_bps, refund_fee) = (self.def.fee_bps, self.def.refund_fee_zat);
        let scan = self.scan().await?;
        // The plan declares which envelopes opened; their contents are never public (ADR-0006).
        let notes: Vec<NoteInput> = scan
            .notes
            .iter()
            .filter(|n| lower.is_none_or(|l| n.block_height > l))
            .map(|n| NoteInput {
                note_ref: n.note_ref.clone(),
                bet_fingerprint: n.bet_fingerprint.clone(),
                outcome_index: n.outcome_index,
                amount_zat: n.amount_zat,
                block_height: n.block_height,
                status: if n.is_announcement {
                    BetStatus::InvalidMemo
                } else {
                    n.status_at(cutoff, min_conf)
                },
                is_announcement: n.is_announcement,
                envelope: match declared.get(n.note_ref.as_str()) {
                    Some(i) if i.receiver_commitment.is_some() => EnvelopeResult::Opened {
                        receiver: [0; 43],
                        bet_salt: [0; 8],
                    },
                    Some(i) if i.kind == ItemKind::Unclaimable => EnvelopeResult::Unopenable,
                    _ => EnvelopeResult::None,
                },
            })
            .collect();
        let outcome = match payout::plan(&PlanInput {
            market_id: self.id.clone(),
            plan_kind: plan.plan_kind,
            plan_cutoff_height: cutoff,
            result: input_result,
            fee_bps,
            refund_fee_zat: refund_fee,
            fee_model: FeeModel::default(),
            notes,
        }) {
            Ok(o) => o,
            Err(e) => {
                self.report.fail(
                    &key("plan_items"),
                    &name,
                    format!("The plan cannot be reproduced from public data: {e}"),
                    None,
                );
                return Ok(Some(cutoff));
            }
        };

        let shape = |items: &[hash::PlanItem]| -> BTreeMap<String, Value> {
            items.iter().map(|i| (i.note_ref.clone(), json!({"kind": i.kind, "amount_zat": i.amount_zat, "bet_fingerprint": i.bet_fingerprint, "has_receiver": i.receiver_commitment.is_some()}))).collect()
        };
        let (mine, theirs) = (shape(&outcome.public.items), shape(&plan.items));
        if mine == theirs {
            self.report.pass(
                &key("plan_items"),
                &name,
                format!(
                    "{} items match by kind and amount (receivers are never public)",
                    mine.len()
                ),
            );
        } else {
            let keys: std::collections::BTreeSet<_> = mine.keys().chain(theirs.keys()).collect();
            let differing: Vec<&&String> = keys
                .iter()
                .filter(|k| mine.get(**k) != theirs.get(**k))
                .collect();
            let pick = |m: &BTreeMap<String, Value>| {
                Value::Object(
                    differing
                        .iter()
                        .map(|k| ((***k).clone(), m.get(**k).cloned().unwrap_or(Value::Null)))
                        .collect(),
                )
            };
            self.report.fail(
                &key("plan_items"),
                &name,
                format!(
                    "{} of {} items differ from the recomputation.",
                    differing.len(),
                    keys.len()
                ),
                Some(Diff {
                    verifier: pick(&mine),
                    api: pick(&theirs),
                }),
            );
        }
        self.report.compare(&key("plan_result"), &label("Plan result and operator fee match"), &json!({"result": outcome.public.result, "operator_fee_zat": outcome.public.operator_fee_zat}),
            &json!({"result": plan.result, "operator_fee_zat": plan.operator_fee_zat}), "result and operator fee match");

        let s = &outcome.summary;
        let mine = json!({"winners": s.winners, "refunds": s.refunds, "unclaimable": s.unclaimable, "swept": s.swept, "uneconomic_dust": s.uneconomic_dust,
            "operator_fee_zat": s.operator_fee_zat, "tx_fee_zat": s.tx_fee_zat, "distributed_zat": s.distributed_zat});
        let theirs: Value = published["summary"].clone();
        self.report.compare(
            &key("plan_summary"),
            &label("Plan summary matches"),
            &mine,
            &theirs,
            format!(
                "distributed {} zat, fees {} zat",
                s.distributed_zat, s.tx_fee_zat
            ),
        );
        Ok(Some(cutoff))
    }
}

fn result_of(resolution: &Value) -> Option<PlanResult> {
    match resolution["status"].as_str()? {
        "RESOLVED" => Some(PlanResult::Outcome(
            resolution["winning_outcome_index"].as_u64()? as u32,
        )),
        "VOID" => Some(PlanResult::Void),
        _ => None,
    }
}

// ------------------------------------------------------------ envelope proof

/// The file a bettor exports from My Bets (`envelope_proof` in the WASM module).
#[derive(Clone, Debug, serde::Deserialize)]
pub struct ProofFile {
    pub note_ref: String,
    pub env_esk: String,
    pub receiver: String,
    pub bet_salt: String,
}

fn fixed<const N: usize>(hex_str: &str, what: &str) -> Result<[u8; N]> {
    hex::decode(hex_str)
        .ok()
        .and_then(|b| b.try_into().ok())
        .with_context(|| format!("{what} must be {N} bytes of hex"))
}

/// Verifies an Envelope Proof against the memo found on the verifier's own node (ADR-0006, US-17).
pub async fn envelope_proof<A: Api, N: Node>(
    api: &A,
    node: &N,
    proof: &ProofFile,
    market: Option<&str>,
) -> Result<Report> {
    let market_id = match market {
        Some(m) => m.to_owned(),
        None => {
            let mut found = None;
            for id in api.market_ids().await? {
                let bets = api.bets(&id).await?;
                if bets["bets"]
                    .as_array()
                    .is_some_and(|b| b.iter().any(|b| b["note_ref"] == proof.note_ref))
                {
                    found = Some(id);
                    break;
                }
            }
            found.with_context(|| {
                format!(
                    "no market has a bet with note_ref {}; pass --market <id>",
                    proof.note_ref
                )
            })?
        }
    };
    let tip = node.tip().await?;
    let mut report = Report::new(&market_id, "envelope-proof", tip);
    let api_market = api.market(&market_id).await?;
    let def: MarketDefinition = serde_json::from_value(api_market["market_definition"].clone())
        .context("market_definition")?;
    let rules_hash = hash::rules_hash(&def)?;
    let envelope_pk: [u8; 32] = {
        use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
        URL_SAFE_NO_PAD
            .decode(&def.envelope_pk)
            .ok()
            .and_then(|b| b.try_into().ok())
            .context("envelope_pk in the market definition")?
    };
    let claim = EnvelopeProof {
        env_esk: fixed(&proof.env_esk, "env_esk")?,
        receiver: fixed(&proof.receiver, "receiver")?,
        bet_salt: fixed(&proof.bet_salt, "bet_salt")?,
    };
    let scan = node.scan(&def, &rules_hash, tip).await?;
    let Some(payload) = scan
        .notes
        .iter()
        .find(|n| n.note_ref == proof.note_ref)
        .and_then(|n| n.payload)
    else {
        report.fail(
            "envelope_proof",
            "Envelope Proof verifies",
            format!(
                "Your node has no valid FRST1 memo for note {} in this market.",
                proof.note_ref
            ),
            None,
        );
        return Ok(report);
    };
    match envelope::verify_proof(&claim, &payload, &envelope_pk) {
        Err(e) => report.fail(
            "envelope_proof",
            "Envelope Proof verifies",
            e.to_string(),
            None,
        ),
        Ok(()) => {
            let label = api
                .payouts(&market_id)
                .await?
                .iter()
                .flat_map(|p| p["plan"]["items"].as_array().cloned().unwrap_or_default())
                .find(|i| i["note_ref"] == proof.note_ref)
                .and_then(|i| i["kind"].as_str().map(str::to_owned));
            let detail = match label.as_deref() {
                Some("UNCLAIMABLE") => "The envelope opens with this proof, so the UNCLAIMABLE label for this note is wrong.".to_owned(),
                Some(kind) => format!("The envelope opens with this proof. The plan labels this note {kind}."),
                None => "The envelope opens with this proof. No plan item exists for this note yet.".to_owned(),
            };
            report.pass("envelope_proof", "Envelope Proof verifies", detail);
        }
    }
    Ok(report)
}

// ---------------------------------------------------------------------- thaw

pub struct ThawOutcome {
    pub checks: Report,
    pub thaw: ThawReport,
}

/// Recomputes a Persona's Thaw Report from the verifier's node and public settlement data, and
/// compares it with the API's report when the Persona has thawed (US-13).
pub async fn thaw<A: Api, N: Node>(
    api: &A,
    node: &N,
    k_root: &[u8; 32],
    alias: Option<&str>,
) -> Result<ThawOutcome> {
    let commitment = thaw::commitment(k_root);
    let persona_id = thaw::persona_id(&commitment);
    let tip = node.tip().await?;
    let mut report = Report::new(&persona_id, "thaw", tip);
    let persona = api.persona(&persona_id).await?;

    // Registration height from the registry key on this node, not from the API.
    let registry = api
        .registry()
        .await?
        .context("the API publishes no Persona Registry")?;
    let (uivk, birthday) = (
        registry["uivk"].as_str().context("registry uivk")?,
        registry["birthday_height"]
            .as_u64()
            .context("registry birthday_height")? as u32,
    );
    // The registry UIVK is encoded for the same network as the markets.
    let market_ids = api.market_ids().await?;
    let mut markets = Vec::new();
    for id in &market_ids {
        let market = api.market(id).await?;
        let def: MarketDefinition = serde_json::from_value(market["market_definition"].clone())
            .with_context(|| format!("market {id}"))?;
        markets.push((id.clone(), def));
    }
    let network = match markets.first() {
        Some((_, def)) => def.network.clone(),
        None => node.network_name().await?,
    };
    let registered = node
        .registrations(uivk, &network, birthday, tip)
        .await?
        .into_iter()
        .filter(|r| r.commitment == commitment)
        .map(|r| r.height)
        .min();
    let Some(registered_height) = registered else {
        report.fail(
            "registered",
            "Persona is registered on this chain",
            "No FRSTP1 registration for this key was found on your node.",
            None,
        );
        return Ok(ThawOutcome {
            checks: report,
            thaw: thaw::report(k_root, alias.unwrap_or_default(), 0, &[], &BTreeMap::new()),
        });
    };
    let api_height = persona
        .as_ref()
        .and_then(|p| p["registered_height"].as_u64())
        .map(|h| h as u32);
    match api_height {
        Some(h) => report.compare("registered", "registered_height matches the chain", &registered_height, &h, format!("block {registered_height}")),
        None => report.info("registered", "registered_height matches the chain", format!("Registered at block {registered_height} on your node; the API does not know this Persona.")),
    }

    let thawed = persona.as_ref().is_some_and(|p| p["status"] == "THAWED");
    let alias = persona
        .as_ref()
        .and_then(|p| p["alias"].as_str().map(str::to_owned))
        .or_else(|| alias.map(str::to_owned))
        .unwrap_or_default();

    let at = api.indexer_height().await?;
    report.api_height = Some(at);
    if tip < at {
        bail!("your node is at block {tip} but the API is at {at}; wait for it to sync");
    }
    let mut bets = Vec::new();
    let mut settlements = BTreeMap::new();
    for id in api.market_ids().await? {
        let market = api.market(&id).await?;
        let def: MarketDefinition = serde_json::from_value(market["market_definition"].clone())
            .with_context(|| format!("market {id}"))?;
        let rules = hash::rules_hash(&def)?;
        let scan = node.scan(&def, &rules, at).await?;
        let resolution = api.resolution(&id).await?;
        let plans = api.payouts(&id).await?;
        let parsed: Vec<(String, Plan)> = plans
            .iter()
            .filter_map(|p| {
                Some((
                    p["status"].as_str()?.to_owned(),
                    serde_json::from_value(p["plan"].clone()).ok()?,
                ))
            })
            .collect();
        let items = PlanItems::from_plans(parsed.iter().map(|(s, p)| (s.as_str(), p)));
        let view = resolution["status"].as_str().map(|status| ResolutionView {
            status,
            winning_outcome_index: resolution["winning_outcome_index"]
                .as_u64()
                .map(|w| w as u32),
        });
        let market_id: [u8; 16] = fixed(&def.market_id, "market_id")?;
        for n in scan.notes.iter().filter(|n| !n.is_announcement) {
            let (Some(payload), Some(outcome)) = (&n.payload, n.outcome_index) else {
                continue;
            };
            if !payload.header.has_thaw_tag() {
                continue;
            }
            let Ok(outcome_u8) = u8::try_from(outcome) else {
                continue;
            };
            let status = serde_json::to_value(n.status_at(at, def.min_confirmations))?;
            settlements.insert(
                n.note_ref.clone(),
                settle::settlement(
                    status.as_str().unwrap_or_default(),
                    Some(outcome),
                    &n.note_ref,
                    view,
                    &items,
                ),
            );
            bets.push(ReportBet {
                note_ref: n.note_ref.clone(),
                bet: TaggedBet {
                    market_id,
                    outcome_index: outcome_u8,
                    amount_zat: n.amount_zat,
                    env_epk: payload.env_epk,
                    has_thaw_tag: true,
                    thaw_tag: payload.header.thaw_tag,
                },
                block_height: n.block_height,
                tx_index: n.tx_index,
                action_index: n.action_index,
            });
        }
    }
    let mine = thaw::report(k_root, &alias, registered_height, &bets, &settlements);
    if thawed {
        match api.thaw_report(&persona_id).await? {
            Some(theirs) => report.compare(
                "thaw_report",
                "Thaw Report is identical to the API's",
                &serde_json::to_value(&mine)?,
                &theirs,
                format!("{} bets, {} settled", mine.bets.len(), mine.stats.settled),
            ),
            None => report.fail(
                "thaw_report",
                "Thaw Report is identical to the API's",
                "The Persona is thawed but the API serves no report.",
                None,
            ),
        }
    } else {
        report.info("thaw_report", "Thaw Report is identical to the API's", format!("This Persona has not thawed, so there is no API report to compare. Computed locally: {} bets, {} settled.", mine.bets.len(), mine.stats.settled));
    }
    Ok(ThawOutcome {
        checks: report,
        thaw: mine,
    })
}

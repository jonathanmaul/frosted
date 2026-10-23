//! Committee-Resolved markets (docs/PROJECT_SPEC_FROSTED.md F-11, UF-4, J-6).
//!
//! Two different members agree on an outcome and an evidence URL, a dispute window of
//! `dispute_window_blocks` follows, and the market resolves unless someone withdraws. Without a
//! decision for `void_after_blocks` after `resolve_height`, every counted bet is refunded.
//!
//! Votes are labels, not signatures (ADR-0007): in the demo one person holds all three member keys
//! (ADR-0008), so the dispute window is a simulation and the README says so.
//!
//! `apply` and `tick` are pure so the rules can be tested without a node; `App` persists the result
//! in `coordinator.db` and publishes it as the market's `Resolution`.

use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::{Row, SqlitePool};
use zcash_glass::hash::{MarketDefinition, ResolutionParams};

use crate::service::App;

pub const MEMBERS: [&str; 3] = ["member-1", "member-2", "member-3"];
const MAX_URL_LEN: usize = 2048;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vote {
    pub member_id: String,
    pub action: String,
    pub outcome_index: Option<u32>,
    pub evidence_url: Option<String>,
    pub created_at: u64,
}

/// The current round: `PROPOSE` (and `CONFIRM`) votes since the last withdrawal.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Round {
    pub votes: Vec<Vote>,
    /// Block at which the second member confirmed; the dispute window runs from here.
    pub proposed_height: Option<u32>,
}

#[derive(Clone, Copy, Debug)]
pub struct Rules {
    pub resolve_height: u32,
    pub dispute_window_blocks: u32,
    pub void_after_blocks: u32,
    pub outcomes: u32,
}

impl Rules {
    pub fn of(def: &MarketDefinition) -> Result<Self> {
        let ResolutionParams::Committee(c) = &def.resolution_params else {
            bail!("NOT_COMMITTEE_MARKET");
        };
        Ok(Rules {
            resolve_height: def.resolve_height,
            dispute_window_blocks: c.dispute_window_blocks,
            void_after_blocks: c.void_after_blocks,
            outcomes: def.outcomes.len() as u32,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Propose {
        member: String,
        outcome: u32,
        evidence_url: String,
    },
    Confirm {
        member: String,
        outcome: u32,
        evidence_url: String,
    },
    Withdraw {
        member: String,
    },
}

/// What a command did to the round.
#[derive(Debug, PartialEq, Eq)]
pub struct Applied {
    pub round: Round,
    /// The command starts a new round (a proposal, or a withdrawal clearing the votes).
    pub new_round: bool,
    pub record: Vote,
}

fn member_ok(member: &str) -> Result<()> {
    MEMBERS
        .contains(&member)
        .then_some(())
        .ok_or_else(|| anyhow!("COMMITTEE_INVALID_VOTE"))
}

fn evidence_ok(url: &str) -> bool {
    url.len() <= MAX_URL_LEN
        && (url.starts_with("https://") || url.starts_with("http://"))
        && url.len() > "https://".len()
        && !url.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// Applies one member command at block `tip`. Errors are the API error codes of the operator endpoint.
pub fn apply(round: &Round, cmd: &Command, rules: &Rules, tip: u32, now: u64) -> Result<Applied> {
    let vote = |member: &str, action: &str, outcome: Option<u32>, url: Option<&str>| Vote {
        member_id: member.to_owned(),
        action: action.to_owned(),
        outcome_index: outcome,
        evidence_url: url.map(str::to_owned),
        created_at: now,
    };
    match cmd {
        Command::Propose {
            member,
            outcome,
            evidence_url,
        }
        | Command::Confirm {
            member,
            outcome,
            evidence_url,
        } => {
            member_ok(member)?;
            if tip < rules.resolve_height {
                bail!("COMMITTEE_NOT_OPEN");
            }
            if *outcome >= rules.outcomes || !evidence_ok(evidence_url) {
                bail!("COMMITTEE_INVALID_VOTE");
            }
            if round.proposed_height.is_some() {
                bail!("COMMITTEE_ALREADY_PROPOSED");
            }
            if matches!(cmd, Command::Propose { .. }) {
                let record = vote(member, "PROPOSE", Some(*outcome), Some(evidence_url));
                // A new proposal replaces an unconfirmed one.
                return Ok(Applied {
                    round: Round {
                        votes: vec![record.clone()],
                        proposed_height: None,
                    },
                    new_round: true,
                    record,
                });
            }
            let [proposal] = round.votes.as_slice() else {
                bail!("COMMITTEE_NO_PROPOSAL")
            };
            if &proposal.member_id == member {
                bail!("COMMITTEE_SAME_MEMBER");
            }
            if proposal.outcome_index != Some(*outcome)
                || proposal.evidence_url.as_deref() != Some(evidence_url)
            {
                bail!("COMMITTEE_MISMATCH");
            }
            let record = vote(member, "CONFIRM", Some(*outcome), Some(evidence_url));
            let mut votes = round.votes.clone();
            votes.push(record.clone());
            Ok(Applied {
                round: Round {
                    votes,
                    proposed_height: Some(tip),
                },
                new_round: false,
                record,
            })
        }
        Command::Withdraw { member } => {
            member_ok(member)?;
            if round.votes.is_empty() {
                bail!("COMMITTEE_NO_PROPOSAL");
            }
            // Any member may withdraw, including one who did not vote. The votes are cleared publicly.
            Ok(Applied {
                round: Round::default(),
                new_round: true,
                record: vote(member, "WITHDRAW", None, None),
            })
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    Resolve { outcome: u32 },
    Void,
}

/// J-6: what the timer decides at block `tip`, if anything.
pub fn tick(round: &Round, rules: &Rules, tip: u32) -> Option<Decision> {
    match round.proposed_height {
        Some(h) if tip >= h.saturating_add(rules.dispute_window_blocks) => round
            .votes
            .first()?
            .outcome_index
            .map(|outcome| Decision::Resolve { outcome }),
        Some(_) => None,
        None if tip >= rules.resolve_height.saturating_add(rules.void_after_blocks) => {
            Some(Decision::Void)
        }
        None => None,
    }
}

/// Public `Resolution` body for the round's current state.
pub fn body(market_id: &str, round: &Round, hash: Option<&str>) -> Value {
    let evidence = match (round.proposed_height, hash) {
        (Some(h), Some(hash)) => json!({"source": "committee", "height": h, "block_hash": hash}),
        _ => Value::Null,
    };
    json!({
        "market_id": market_id,
        "status": if round.proposed_height.is_some() { "PROPOSED" } else { "AWAITING_RESOLUTION" },
        "winning_outcome_index": Value::Null,
        "evidence": evidence,
        "votes": round.votes,
    })
}

fn terminal(status: &str) -> bool {
    matches!(status, "RESOLVED" | "VOID" | "PAYING" | "SETTLED")
}

impl App {
    async fn committee_market(&self, id: &str) -> Result<(MarketDefinition, String)> {
        let row = sqlx::query("SELECT definition_json,status FROM market WHERE market_id=?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .context("MARKET_NOT_FOUND")?;
        let def: MarketDefinition = serde_json::from_str(&row.get::<String, _>(0))?;
        if !matches!(def.resolution_params, ResolutionParams::Committee(_)) {
            bail!("NOT_COMMITTEE_MARKET");
        }
        Ok((def, row.get(1)))
    }

    /// Current round number, its votes, and the block hash recorded when it was confirmed.
    async fn committee_round(&self, id: &str) -> Result<(i64, Round, Option<String>)> {
        let state = sqlx::query(
            "SELECT round,proposed_height,proposed_hash FROM committee_round WHERE market_id=?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        let (number, height, hash) = match &state {
            Some(r) => (
                r.get::<i64, _>(0),
                r.get::<Option<i64>, _>(1),
                r.get::<Option<String>, _>(2),
            ),
            None => (0, None, None),
        };
        let rows = sqlx::query("SELECT member_id,action,outcome_index,evidence_url,created_at FROM committee_vote WHERE market_id=? AND round=? AND action IN ('PROPOSE','CONFIRM') ORDER BY id")
            .bind(id)
            .bind(number)
            .fetch_all(&self.pool)
            .await?;
        let votes = rows
            .iter()
            .map(|r| Vote {
                member_id: r.get(0),
                action: r.get(1),
                outcome_index: r.get::<Option<i64>, _>(2).map(|o| o as u32),
                evidence_url: r.get(3),
                created_at: r.get::<i64, _>(4) as u64,
            })
            .collect();
        Ok((
            number,
            Round {
                votes,
                proposed_height: height.map(|h| h as u32),
            },
            hash,
        ))
    }

    async fn set_resolution(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
        id: &str,
        body: &Value,
    ) -> Result<()> {
        sqlx::query("INSERT INTO resolution(market_id,body_json,pushed) VALUES(?,?,0) ON CONFLICT(market_id) DO UPDATE SET body_json=excluded.body_json,pushed=0")
            .bind(id)
            .bind(serde_json::to_string(body)?)
            .execute(&mut **tx)
            .await?;
        Ok(())
    }

    /// `frosted-operator resolution propose|confirm|withdraw` (UF-4).
    pub async fn committee_vote(&self, id: &str, cmd: Command) -> Result<Value> {
        let _guard = self.operation.lock().await;
        let (def, status) = self.committee_market(id).await?;
        if status == "ANNOUNCING" || terminal(&status) {
            bail!("COMMITTEE_NOT_OPEN");
        }
        let rules = Rules::of(&def)?;
        let tip = self.tip().await?;
        let (number, round, _) = self.committee_round(id).await?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        let applied = apply(&round, &cmd, &rules, tip, now)?;
        // The block that starts the dispute window is public evidence of when it started.
        let hash = match applied.round.proposed_height {
            Some(h) => Some(
                self.rpc_params("getblockhash", json!([h]))
                    .await?
                    .as_str()
                    .context("NODE_UNAVAILABLE")?
                    .to_owned(),
            ),
            None => None,
        };
        let target = if applied.new_round {
            number + 1
        } else {
            number
        };
        let mut tx = self.pool.begin().await?;
        // A withdrawal belongs to the round it closes; a proposal opens the next one.
        let record_round = if matches!(cmd, Command::Withdraw { .. }) {
            number
        } else {
            target
        };
        let v = &applied.record;
        sqlx::query("INSERT INTO committee_vote(market_id,round,member_id,action,outcome_index,evidence_url,created_at) VALUES(?,?,?,?,?,?,?)")
            .bind(id).bind(record_round).bind(&v.member_id).bind(&v.action)
            .bind(v.outcome_index.map(i64::from)).bind(&v.evidence_url).bind(v.created_at as i64)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO committee_round(market_id,round,proposed_height,proposed_hash) VALUES(?,?,?,?) ON CONFLICT(market_id) DO UPDATE SET round=excluded.round,proposed_height=excluded.proposed_height,proposed_hash=excluded.proposed_hash")
            .bind(id).bind(target).bind(applied.round.proposed_height.map(i64::from)).bind(&hash)
            .execute(&mut *tx)
            .await?;
        let published = body(id, &applied.round, hash.as_deref());
        self.set_resolution(&mut tx, id, &published).await?;
        tx.commit().await?;
        Ok(published)
    }

    /// Current public body plus the dispute-window countdown, for `resolution status`.
    pub async fn committee_status(&self, id: &str) -> Result<Value> {
        let (def, _) = self.committee_market(id).await?;
        let rules = Rules::of(&def)?;
        let tip = self.tip().await?;
        let (_, round, hash) = self.committee_round(id).await?;
        let published: Option<String> =
            sqlx::query_scalar("SELECT body_json FROM resolution WHERE market_id=?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;
        let resolution = match published {
            Some(b) => serde_json::from_str(&b)?,
            None => body(id, &round, hash.as_deref()),
        };
        let blocks_left = round
            .proposed_height
            .map(|h| i64::from(h) + i64::from(rules.dispute_window_blocks) - i64::from(tip));
        let void_in =
            i64::from(rules.resolve_height) + i64::from(rules.void_after_blocks) - i64::from(tip);
        Ok(
            json!({"tip": tip, "resolution": resolution, "dispute_window_blocks_left": blocks_left, "void_after_blocks_left": void_in}),
        )
    }

    /// J-6 `committee_timer`: runs every tick. `indexer` is the public index, used to void a
    /// resolution whose winning outcome has no counted money (same rule as chain markets).
    pub(crate) async fn committee_tick(&self, tip: u32, indexer: &SqlitePool) -> Result<()> {
        let rows = sqlx::query("SELECT market_id,definition_json,status FROM market WHERE status NOT IN ('ANNOUNCING','VOID','RESOLVED','PAYING','SETTLED')")
            .fetch_all(&self.pool)
            .await?;
        for row in rows {
            let id: String = row.get(0);
            let def: MarketDefinition = serde_json::from_str(&row.get::<String, _>(1))?;
            let Ok(rules) = Rules::of(&def) else { continue };
            let (_, round, hash) = self.committee_round(&id).await?;
            let Some(decision) = tick(&round, &rules, tip) else {
                continue;
            };
            let mut published = body(&id, &round, hash.as_deref());
            let outcome = match decision {
                Decision::Resolve { outcome } => {
                    published["status"] = json!("RESOLVED");
                    published["winning_outcome_index"] = json!(outcome);
                    let backed: i64 = sqlx::query_scalar("SELECT COALESCE(SUM(amount_zat),0) FROM bet WHERE market_id=? AND status='COUNTED' AND outcome_index=?")
                        .bind(&id).bind(i64::from(outcome)).fetch_one(indexer).await?;
                    zcash_glass::resolution::void_if_unbacked(&mut published, backed as u64);
                    published["status"]
                        .as_str()
                        .unwrap_or("RESOLVED")
                        .to_owned()
                }
                Decision::Void => {
                    published["status"] = json!("VOID");
                    "VOID".to_owned()
                }
            };
            let mut tx = self.pool.begin().await?;
            self.set_resolution(&mut tx, &id, &published).await?;
            sqlx::query("UPDATE market SET status=? WHERE market_id=?")
                .bind(&outcome)
                .bind(&id)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RULES: Rules = Rules {
        resolve_height: 100,
        dispute_window_blocks: 10,
        void_after_blocks: 50,
        outcomes: 2,
    };
    const URL: &str = "https://example.org/evidence";

    fn propose(member: &str, outcome: u32, url: &str) -> Command {
        Command::Propose {
            member: member.into(),
            outcome,
            evidence_url: url.into(),
        }
    }
    fn confirm(member: &str, outcome: u32, url: &str) -> Command {
        Command::Confirm {
            member: member.into(),
            outcome,
            evidence_url: url.into(),
        }
    }
    fn run(round: &Round, cmd: Command, tip: u32) -> Result<Round> {
        apply(round, &cmd, &RULES, tip, 1).map(|a| a.round)
    }
    fn code(r: Result<Round>) -> String {
        r.unwrap_err().to_string()
    }

    #[test]
    fn two_members_agree_and_the_window_starts_at_the_confirming_block() {
        let proposed = run(&Round::default(), propose("member-1", 0, URL), 100).unwrap();
        assert_eq!((proposed.votes.len(), proposed.proposed_height), (1, None));
        let confirmed = run(&proposed, confirm("member-2", 0, URL), 103).unwrap();
        assert_eq!(confirmed.proposed_height, Some(103));
        assert_eq!(
            confirmed
                .votes
                .iter()
                .map(|v| v.action.as_str())
                .collect::<Vec<_>>(),
            ["PROPOSE", "CONFIRM"]
        );
        let b = body("m", &confirmed, Some("hash"));
        assert_eq!(b["status"], "PROPOSED");
        assert_eq!(
            b["evidence"],
            json!({"source":"committee","height":103,"block_hash":"hash"})
        );
        assert_eq!(b["votes"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn disagreement_same_member_and_missing_proposal_are_rejected() {
        let proposed = run(&Round::default(), propose("member-1", 0, URL), 100).unwrap();
        assert_eq!(
            code(run(&proposed, confirm("member-2", 1, URL), 101)),
            "COMMITTEE_MISMATCH"
        );
        assert_eq!(
            code(run(
                &proposed,
                confirm("member-2", 0, "https://example.org/other"),
                101
            )),
            "COMMITTEE_MISMATCH"
        );
        assert_eq!(
            code(run(&proposed, confirm("member-1", 0, URL), 101)),
            "COMMITTEE_SAME_MEMBER"
        );
        assert_eq!(
            code(run(&Round::default(), confirm("member-2", 0, URL), 101)),
            "COMMITTEE_NO_PROPOSAL"
        );
        // The rejected confirmation changed nothing: the market stays AWAITING_RESOLUTION.
        assert_eq!(body("m", &proposed, None)["status"], "AWAITING_RESOLUTION");
    }

    #[test]
    fn nothing_before_resolve_height_and_inputs_are_validated() {
        assert_eq!(
            code(run(&Round::default(), propose("member-1", 0, URL), 99)),
            "COMMITTEE_NOT_OPEN"
        );
        for (member, outcome, url) in [
            ("mallory", 0, URL),
            ("member-1", 2, URL),
            ("member-1", 0, "javascript:alert(1)"),
            ("member-1", 0, "https://a b"),
            ("member-1", 0, "https://"),
        ] {
            assert_eq!(
                code(run(&Round::default(), propose(member, outcome, url), 100)),
                "COMMITTEE_INVALID_VOTE",
                "{member} {outcome} {url}"
            );
        }
        assert_eq!(
            code(run(
                &Round::default(),
                propose("member-1", 0, &format!("https://x/{}", "a".repeat(3000))),
                100
            )),
            "COMMITTEE_INVALID_VOTE"
        );
    }

    #[test]
    fn a_new_proposal_replaces_an_unconfirmed_one_but_not_a_confirmed_one() {
        let first = apply(
            &Round::default(),
            &propose("member-1", 0, URL),
            &RULES,
            100,
            1,
        )
        .unwrap();
        assert!(first.new_round);
        let second = apply(&first.round, &propose("member-3", 1, URL), &RULES, 101, 2).unwrap();
        assert_eq!(second.round.votes.len(), 1);
        assert_eq!(second.round.votes[0].outcome_index, Some(1));
        let confirmed = run(&second.round, confirm("member-2", 1, URL), 102).unwrap();
        assert_eq!(
            code(run(&confirmed, propose("member-1", 0, URL), 103)),
            "COMMITTEE_ALREADY_PROPOSED"
        );
        assert_eq!(
            code(run(&confirmed, confirm("member-3", 1, URL), 103)),
            "COMMITTEE_ALREADY_PROPOSED"
        );
    }

    #[test]
    fn any_member_can_withdraw_and_the_votes_are_cleared() {
        let confirmed = run(
            &run(&Round::default(), propose("member-1", 0, URL), 100).unwrap(),
            confirm("member-2", 0, URL),
            101,
        )
        .unwrap();
        let withdrawn = apply(
            &confirmed,
            &Command::Withdraw {
                member: "member-3".into(),
            },
            &RULES,
            105,
            3,
        )
        .unwrap();
        assert!(withdrawn.new_round);
        assert_eq!(withdrawn.round, Round::default());
        assert_eq!(
            body("m", &withdrawn.round, None),
            json!({"market_id":"m","status":"AWAITING_RESOLUTION","winning_outcome_index":null,"evidence":null,"votes":[]})
        );
        assert_eq!(
            code(run(
                &Round::default(),
                Command::Withdraw {
                    member: "member-1".into()
                },
                105
            )),
            "COMMITTEE_NO_PROPOSAL"
        );
        assert_eq!(
            code(run(
                &confirmed,
                Command::Withdraw {
                    member: "nobody".into()
                },
                105
            )),
            "COMMITTEE_INVALID_VOTE"
        );
    }

    #[test]
    fn timer_resolves_after_the_dispute_window_and_voids_without_a_decision() {
        let confirmed = run(
            &run(&Round::default(), propose("member-1", 1, URL), 100).unwrap(),
            confirm("member-2", 1, URL),
            110,
        )
        .unwrap();
        assert_eq!(tick(&confirmed, &RULES, 119), None, "inside the window");
        assert_eq!(
            tick(&confirmed, &RULES, 120),
            Some(Decision::Resolve { outcome: 1 })
        );
        // A proposed market resolves even after the void deadline: the window rules.
        assert_eq!(
            tick(&confirmed, &RULES, 500),
            Some(Decision::Resolve { outcome: 1 })
        );
        let idle = Round::default();
        assert_eq!(tick(&idle, &RULES, 149), None);
        assert_eq!(tick(&idle, &RULES, 150), Some(Decision::Void));
        let pending = run(&idle, propose("member-1", 0, URL), 120).unwrap();
        assert_eq!(
            tick(&pending, &RULES, 150),
            Some(Decision::Void),
            "an unconfirmed proposal does not stop the void"
        );
    }

    #[test]
    fn rules_require_a_committee_market() {
        let def: MarketDefinition = serde_json::from_value(json!({"version":1,"params_version":1,"network":"regtest","market_id":"00112233445566778899aabbccddeeff","question":"q",
            "outcomes":[{"outcome_index":0,"label":"YES","address":"a"},{"outcome_index":1,"label":"NO","address":"b"}],
            "resolution_type":"POOL_THRESHOLD","resolution_params":{"pool":"ironwood","comparator":"gte","threshold_zat":1},
            "birthday_height":1,"close_height":5,"resolve_height":10,"fee_bps":100,"min_bet_zat":1,"refund_fee_zat":1,"min_confirmations":1,
            "sweep_threshold_zat":1,"announce_timeout_blocks":1,"uivk":"u","envelope_pk":"p","treasury_address":"t"})).unwrap();
        assert_eq!(
            Rules::of(&def).unwrap_err().to_string(),
            "NOT_COMMITTEE_MARKET"
        );
    }
}

#[cfg(test)]
mod app_tests {
    //! The coordinator side of UF-4 and J-6: persistence, the timer, and what is pushed to the API.
    use super::*;
    use crate::{
        config::Config,
        secrets::{Secrets, write_private_new},
    };
    use axum::{Json, Router, extract::State, routing::post};
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicU32, Ordering},
    };

    const ID: &str = "00112233445566778899aabbccddeeff";
    const URL: &str = "https://example.org/evidence";

    #[derive(Default)]
    struct Chain {
        tip: AtomicU32,
        pushes: Mutex<Vec<Value>>,
    }

    struct Fixture {
        app: Arc<App>,
        chain: Arc<Chain>,
        indexer: SqlitePool,
        _dir: tempfile::TempDir,
        server: tokio::task::JoinHandle<()>,
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            self.server.abort();
        }
    }

    async fn rpc(State(c): State<Arc<Chain>>, Json(body): Json<Value>) -> Json<Value> {
        Json(match body["method"].as_str().unwrap() {
            "getblockcount" => json!({"result": c.tip.load(Ordering::SeqCst), "error": null}),
            "getblockhash" => {
                json!({"result": format!("hash-{}", body["params"][0]), "error": null})
            }
            other => panic!("unexpected RPC {other}"),
        })
    }
    async fn ingest(State(c): State<Arc<Chain>>, Json(body): Json<Value>) {
        c.pushes.lock().unwrap().push(body);
    }

    fn definition(kind: &str) -> MarketDefinition {
        let params = if kind == "COMMITTEE" {
            json!({"criteria":"c","evidence_source":"e","dispute_window_blocks":10,"void_after_blocks":50})
        } else {
            json!({"pool":"ironwood","comparator":"gte","threshold_zat":1})
        };
        serde_json::from_value(json!({"version":1,"params_version":1,"network":"regtest","market_id":ID,"question":"Will the committee say YES?",
            "outcomes":[{"outcome_index":0,"label":"YES","address":"u1"},{"outcome_index":1,"label":"NO","address":"u2"}],
            "resolution_type":kind,"resolution_params":params,"birthday_height":1,"close_height":10,"resolve_height":20,"fee_bps":100,
            "min_bet_zat":1000000,"refund_fee_zat":5000,"min_confirmations":3,"sweep_threshold_zat":5000,"announce_timeout_blocks":100,
            "uivk":"uivk","envelope_pk":"key","treasury_address":"treasury"})).unwrap()
    }

    async fn fixture(kind: &str, tip: u32) -> Fixture {
        let chain = Arc::new(Chain::default());
        chain.tip.store(tip, Ordering::SeqCst);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let bind = listener.local_addr().unwrap();
        let server = tokio::spawn({
            let router = Router::new()
                .route("/rpc", post(rpc))
                .route("/v1/ingest/markets/{id}/resolution", post(ingest))
                .with_state(chain.clone());
            async move { axum::serve(listener, router).await.unwrap() }
        });
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_private_new(&root.join("master"), &[8; 32]).unwrap();
        let master = Secrets::load(&root.join("master")).unwrap();
        write_private_new(&root.join("coordinator"), &[9; 32]).unwrap();
        write_private_new(
            &root.join("envelope"),
            &master.seal("envelope_sk", &[10; 32]).unwrap(),
        )
        .unwrap();
        write_private_new(
            &root.join("hot"),
            &master.seal("hot_wallet_seed", &[11; 32]).unwrap(),
        )
        .unwrap();
        let app = App::new(Config {
            bind,
            network: "regtest".into(),
            db: root.join("coordinator.db"),
            indexer_db: root.join("indexer.db"),
            secrets_key: root.join("master"),
            coordinator_key: root.join("coordinator"),
            envelope_key: root.join("envelope"),
            hot_seed: root.join("hot"),
            treasury: "not used".into(),
            rpc_url: format!("http://{bind}/rpc"),
            zaino_url: "not used".into(),
            ingest_url: format!("http://{bind}/v1/ingest").parse().unwrap(),
            helper: root.join("helper"),
            min_bet_zat: 1_000_000,
            min_confirmations: 3,
            max_mainnet_demo_zat: 50_000_000,
            sync_interval: std::time::Duration::from_secs(30),
        })
        .await
        .unwrap();
        sqlx::query("INSERT INTO market(market_id,definition_json,rules_hash,nk_enc,rivk_enc,ufvk_enc,wallet_enc,status) VALUES(?,?,?,?,?,?,?,'OPEN')")
            .bind(ID).bind(serde_json::to_string(&definition(kind)).unwrap()).bind("rules")
            .bind(vec![0]).bind(vec![0]).bind(vec![0]).bind(vec![0])
            .execute(&app.pool).await.unwrap();
        let indexer = SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query(
            "CREATE TABLE bet(market_id TEXT,status TEXT,outcome_index INTEGER,amount_zat INTEGER)",
        )
        .execute(&indexer)
        .await
        .unwrap();
        Fixture {
            app,
            chain,
            indexer,
            _dir: dir,
            server,
        }
    }

    impl Fixture {
        fn set_tip(&self, tip: u32) {
            self.chain.tip.store(tip, Ordering::SeqCst);
        }
        async fn vote(&self, c: Command) -> Result<Value> {
            self.app.committee_vote(ID, c).await
        }
        async fn tick(&self) {
            let tip = self.chain.tip.load(Ordering::SeqCst);
            self.app.committee_tick(tip, &self.indexer).await.unwrap();
        }
        async fn stored_status(&self) -> String {
            sqlx::query_scalar("SELECT status FROM market")
                .fetch_one(&self.app.pool)
                .await
                .unwrap()
        }
        async fn count_bet(&self, outcome: i64, amount: i64) {
            sqlx::query("INSERT INTO bet VALUES(?,'COUNTED',?,?)")
                .bind(ID)
                .bind(outcome)
                .bind(amount)
                .execute(&self.indexer)
                .await
                .unwrap();
        }
    }
    fn propose(member: &str, outcome: u32) -> Command {
        Command::Propose {
            member: member.into(),
            outcome,
            evidence_url: URL.into(),
        }
    }
    fn confirm(member: &str, outcome: u32) -> Command {
        Command::Confirm {
            member: member.into(),
            outcome,
            evidence_url: URL.into(),
        }
    }
    fn err(r: Result<Value>) -> String {
        r.unwrap_err().to_string()
    }

    #[tokio::test]
    async fn uf4_propose_confirm_withdraw_then_resolve_after_the_window() {
        let f = fixture("COMMITTEE", 19).await;
        assert_eq!(
            err(f.vote(propose("member-1", 0)).await),
            "COMMITTEE_NOT_OPEN",
            "before resolve_height"
        );
        f.set_tip(20);
        let pending = f.vote(propose("member-1", 0)).await.unwrap();
        assert_eq!(pending["status"], "AWAITING_RESOLUTION");
        assert_eq!(pending["votes"].as_array().unwrap().len(), 1);
        assert_eq!(
            err(f.vote(confirm("member-2", 1)).await),
            "COMMITTEE_MISMATCH"
        );
        assert_eq!(
            f.app.committee_status(ID).await.unwrap()["resolution"]["status"],
            "AWAITING_RESOLUTION",
            "a disagreement changes nothing"
        );

        f.set_tip(21);
        let proposed = f.vote(confirm("member-2", 0)).await.unwrap();
        assert_eq!(proposed["status"], "PROPOSED");
        assert_eq!(
            proposed["evidence"],
            json!({"source":"committee","height":21,"block_hash":"hash-21"})
        );
        assert_eq!(proposed["votes"].as_array().unwrap().len(), 2);
        f.set_tip(25);
        let status = f.app.committee_status(ID).await.unwrap();
        assert_eq!(status["dispute_window_blocks_left"], 6);

        // A withdrawal inside the window returns the market to AWAITING_RESOLUTION with the votes cleared.
        let cleared = f
            .vote(Command::Withdraw {
                member: "member-3".into(),
            })
            .await
            .unwrap();
        assert_eq!(
            (
                cleared["status"].as_str(),
                cleared["votes"].as_array().unwrap().len()
            ),
            (Some("AWAITING_RESOLUTION"), 0)
        );
        assert_eq!(f.stored_status().await, "OPEN");

        // Propose again; the window now runs from the new confirmation.
        f.vote(propose("member-3", 1)).await.unwrap();
        f.vote(confirm("member-1", 1)).await.unwrap();
        f.tick().await;
        assert_eq!(f.stored_status().await, "OPEN", "inside the window");
        f.set_tip(34);
        f.tick().await;
        assert_eq!(f.stored_status().await, "OPEN", "one block short");
        f.count_bet(1, 5_000_000).await;
        f.set_tip(35);
        f.tick().await;
        assert_eq!(f.stored_status().await, "RESOLVED");
        let (resolved, pushed): (String, i64) =
            sqlx::query_as("SELECT body_json,pushed FROM resolution")
                .fetch_one(&f.app.pool)
                .await
                .map(|r: (String, i64)| r)
                .unwrap();
        let body: Value = serde_json::from_str(&resolved).unwrap();
        assert_eq!(
            (
                body["status"].as_str(),
                body["winning_outcome_index"].as_u64(),
                pushed
            ),
            (Some("RESOLVED"), Some(1), 0)
        );
        assert_eq!(body["votes"].as_array().unwrap().len(), 2);

        // Terminal: no more votes, and what the API sees is the same body.
        assert_eq!(
            err(f
                .vote(Command::Withdraw {
                    member: "member-1".into()
                })
                .await),
            "COMMITTEE_NOT_OPEN"
        );
        f.app.publish_resolutions().await.unwrap();
        let pushes = f.chain.pushes.lock().unwrap();
        assert_eq!(pushes.last().unwrap(), &body);
        let statuses: Vec<_> = pushes
            .iter()
            .map(|p| p["status"].as_str().unwrap())
            .collect();
        assert_eq!(
            statuses,
            ["RESOLVED"],
            "only the latest body is pushed, once"
        );
    }

    #[tokio::test]
    async fn every_change_is_pushed_so_the_api_follows_the_votes() {
        let f = fixture("COMMITTEE", 20).await;
        f.vote(propose("member-1", 0)).await.unwrap();
        f.app.publish_resolutions().await.unwrap();
        f.vote(confirm("member-2", 0)).await.unwrap();
        f.app.publish_resolutions().await.unwrap();
        f.vote(Command::Withdraw {
            member: "member-1".into(),
        })
        .await
        .unwrap();
        f.app.publish_resolutions().await.unwrap();
        f.app.publish_resolutions().await.unwrap(); // nothing new: not pushed twice
        let pushes = f.chain.pushes.lock().unwrap();
        let seen: Vec<_> = pushes
            .iter()
            .map(|p| {
                (
                    p["status"].as_str().unwrap(),
                    p["votes"].as_array().unwrap().len(),
                )
            })
            .collect();
        assert_eq!(
            seen,
            [
                ("AWAITING_RESOLUTION", 1),
                ("PROPOSED", 2),
                ("AWAITING_RESOLUTION", 0)
            ]
        );
    }

    #[tokio::test]
    async fn no_decision_for_void_after_blocks_voids_the_market() {
        let f = fixture("COMMITTEE", 69).await;
        f.tick().await;
        assert_eq!(f.stored_status().await, "OPEN");
        f.set_tip(70); // resolve_height 20 + void_after_blocks 50
        f.tick().await;
        assert_eq!(f.stored_status().await, "VOID");
        f.app.publish_resolutions().await.unwrap();
        let voided = {
            let pushes = f.chain.pushes.lock().unwrap();
            (
                pushes[0]["status"].as_str().map(str::to_owned),
                pushes[0]["winning_outcome_index"].is_null(),
            )
        };
        assert_eq!(voided, (Some("VOID".to_owned()), true));
        assert_eq!(
            err(f.vote(propose("member-1", 0)).await),
            "COMMITTEE_NOT_OPEN"
        );
    }

    #[tokio::test]
    async fn a_decision_nobody_bet_on_becomes_void_like_chain_markets() {
        let f = fixture("COMMITTEE", 20).await;
        f.count_bet(0, 5_000_000).await; // money only on YES
        f.vote(propose("member-1", 1)).await.unwrap();
        f.vote(confirm("member-2", 1)).await.unwrap();
        f.set_tip(30);
        f.tick().await;
        assert_eq!(f.stored_status().await, "VOID");
    }

    #[tokio::test]
    async fn chain_verified_markets_and_unknown_markets_have_no_committee() {
        let f = fixture("POOL_THRESHOLD", 20).await;
        assert_eq!(
            err(f.vote(propose("member-1", 0)).await),
            "NOT_COMMITTEE_MARKET"
        );
        assert_eq!(
            err(f.app.committee_vote("nope", propose("member-1", 0)).await),
            "MARKET_NOT_FOUND"
        );
        f.set_tip(500);
        f.tick().await; // the timer ignores markets that are not committee-resolved
        assert_eq!(f.stored_status().await, "OPEN");
    }
}

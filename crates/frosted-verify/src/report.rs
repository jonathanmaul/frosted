//! Check results. The same JSON is printed by `--json`, served by `GET /v1/markets/{id}/verify`
//! and shown on S-10.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Status {
    Pass,
    Fail,
    /// The check could not run (nothing published yet, or out of scope for this market type).
    Skip,
    /// Something to read, not a verdict (for example `UNCLAIMABLE` items).
    Info,
}

/// What the verifier computed versus what the API claims.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Diff {
    pub verifier: Value,
    pub api: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Check {
    pub id: String,
    pub name: String,
    pub status: Status,
    pub detail: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub diff: Option<Diff>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub market_id: String,
    pub command: String,
    pub node_tip: u32,
    /// Height the API's odds were computed at (the indexer height).
    pub api_height: Option<u32>,
    pub checks: Vec<Check>,
    /// True when no check failed.
    pub ok: bool,
}

impl Report {
    pub fn new(market_id: &str, command: &str, node_tip: u32) -> Self {
        Report {
            market_id: market_id.into(),
            command: command.into(),
            node_tip,
            api_height: None,
            checks: Vec::new(),
            ok: true,
        }
    }

    pub fn push(&mut self, id: &str, name: &str, status: Status, detail: impl Into<String>) {
        self.checks.push(Check {
            id: id.into(),
            name: name.into(),
            status,
            detail: detail.into(),
            diff: None,
        });
        self.ok &= status != Status::Fail;
    }

    pub fn pass(&mut self, id: &str, name: &str, detail: impl Into<String>) {
        self.push(id, name, Status::Pass, detail);
    }

    pub fn skip(&mut self, id: &str, name: &str, detail: impl Into<String>) {
        self.push(id, name, Status::Skip, detail);
    }

    pub fn info(&mut self, id: &str, name: &str, detail: impl Into<String>) {
        self.push(id, name, Status::Info, detail);
    }

    pub fn fail(&mut self, id: &str, name: &str, detail: impl Into<String>, diff: Option<Diff>) {
        self.push(id, name, Status::Fail, detail);
        if let Some(last) = self.checks.last_mut() {
            last.diff = diff;
        }
    }

    /// PASS when `verifier == api`, otherwise FAIL with a diff.
    pub fn compare<T: Serialize + PartialEq>(
        &mut self,
        id: &str,
        name: &str,
        verifier: &T,
        api: &T,
        pass_detail: impl Into<String>,
    ) {
        if verifier == api {
            self.pass(id, name, pass_detail);
        } else {
            let diff = Diff {
                verifier: serde_json::to_value(verifier).unwrap_or(Value::Null),
                api: serde_json::to_value(api).unwrap_or(Value::Null),
            };
            self.fail(
                id,
                name,
                "The API's value differs from the verifier's recomputation.",
                Some(diff),
            );
        }
    }

    /// Human-readable lines, one per check, with the diff indented below a failure.
    pub fn render(&self) -> String {
        let mut out = format!(
            "frosted-verify {} {} (node tip {}{})\n",
            self.command,
            self.market_id,
            self.node_tip,
            self.api_height
                .map(|h| format!(", API at {h}"))
                .unwrap_or_default()
        );
        for c in &self.checks {
            let tag = match c.status {
                Status::Pass => "PASS",
                Status::Fail => "FAIL",
                Status::Skip => "SKIP",
                Status::Info => "INFO",
            };
            out += &format!("[{tag}] {}: {}\n", c.name, c.detail);
            if let Some(d) = &c.diff {
                out += &format!(
                    "       verifier: {}\n       api:      {}\n",
                    d.verifier, d.api
                );
            }
        }
        out += if self.ok {
            "RESULT: PASS\n"
        } else {
            "RESULT: FAIL\n"
        };
        out
    }
}

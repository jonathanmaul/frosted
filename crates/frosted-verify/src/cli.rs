//! Command line: parsing, help, and running one invocation against a node and an API.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use serde_json::json;

use crate::{
    checks::{self, Command, ProofFile},
    report::Report,
    rpc::{HttpApi, RpcNode},
};

pub const DEFAULT_NODE: &str = "http://127.0.0.1:8232";
pub const DEFAULT_API: &str = "http://127.0.0.1:8080/v1";

pub const HELP: &str = "\
frosted-verify: audit Frosted from your own node, without trusting the Frosted server.

USAGE
    frosted-verify <COMMAND> [OPTIONS]

COMMANDS
    market <market_id>          rules_hash, Outcome Addresses derive from the UIVK, on-chain announcement
    pools <market_id>           scan your node with the UIVK and recompute the pool per outcome
    resolution <market_id>      recompute the Chain-Verified outcome (POOL_THRESHOLD, UPGRADE_ACTIVE)
    all <market_id>             market + pools + resolution + payout plan (plan_hash and every item)
    thaw <k_root_hex>           recompute a Persona's Thaw Report; compare with the API once thawed
    envelope-proof <file>       verify an Envelope Proof exported from My Bets (disproves UNCLAIMABLE)

OPTIONS
    --node <url>      your zebrad JSON-RPC endpoint        [env FROSTED_NODE_RPC, default http://127.0.0.1:8232]
    --api <url>       the Frosted API to check             [env FROSTED_API_URL, default http://127.0.0.1:8080/v1]
    --json            print machine-readable JSON instead of text
    --alias <name>    thaw: alias to show when the Persona has not thawed yet
    --market <id>     envelope-proof: skip the market search and use this market
    -h, --help        print this help

EXIT CODES
    0   every check passed (INFO and SKIP are not failures)
    1   at least one check failed
    2   usage error, or the node/API could not be read

The verifier reads only your node and the public API; it needs no secret. Scanning starts at
the market's birthday_height, so a long-lived market on a long chain takes a while.
";

#[derive(Debug, PartialEq, Eq)]
pub enum Invocation {
    Help,
    Run(Request),
}

#[derive(Debug, PartialEq, Eq)]
pub struct Request {
    pub action: Action,
    pub node: String,
    pub api: String,
    pub json: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    Market(Command, String),
    Thaw {
        k_root: String,
        alias: Option<String>,
    },
    EnvelopeProof {
        file: PathBuf,
        market: Option<String>,
    },
}

/// `env` supplies the defaults for `--node` and `--api`.
pub fn parse(args: &[String], env: impl Fn(&str) -> Option<String>) -> Result<Invocation> {
    let (mut node, mut api) = (env("FROSTED_NODE_RPC"), env("FROSTED_API_URL"));
    let (mut json, mut alias, mut market) = (false, None, None);
    let mut positional = Vec::new();
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        let mut value = |name: &str| {
            it.next()
                .cloned()
                .with_context(|| format!("{name} needs a value"))
        };
        match arg.as_str() {
            "-h" | "--help" => return Ok(Invocation::Help),
            "--json" => json = true,
            "--node" => node = Some(value("--node")?),
            "--api" => api = Some(value("--api")?),
            "--alias" => alias = Some(value("--alias")?),
            "--market" => market = Some(value("--market")?),
            flag if flag.starts_with('-') => bail!("unknown option {flag}"),
            other => positional.push(other.to_owned()),
        }
    }
    let [command, arg] = positional.as_slice() else {
        if positional.is_empty() {
            bail!("missing command");
        }
        bail!("expected exactly one argument after the command");
    };
    let action = match command.as_str() {
        "market" => Action::Market(Command::Market, arg.clone()),
        "pools" => Action::Market(Command::Pools, arg.clone()),
        "resolution" => Action::Market(Command::Resolution, arg.clone()),
        "all" => Action::Market(Command::All, arg.clone()),
        "thaw" => Action::Thaw {
            k_root: arg.clone(),
            alias,
        },
        "envelope-proof" => Action::EnvelopeProof {
            file: arg.into(),
            market,
        },
        other => bail!("unknown command {other}"),
    };
    Ok(Invocation::Run(Request {
        action,
        node: node.unwrap_or_else(|| DEFAULT_NODE.into()),
        api: api.unwrap_or_else(|| DEFAULT_API.into()),
        json,
    }))
}

/// Runs a request and returns the text or JSON to print and whether every check passed.
pub async fn execute(req: &Request) -> Result<(String, bool)> {
    let node = RpcNode::new(&req.node)?;
    let api = HttpApi::new(&req.api)?;
    match &req.action {
        Action::Market(command, id) => {
            finish(checks::run(&api, &node, id, *command).await?, req.json)
        }
        Action::EnvelopeProof { file, market } => {
            let text = std::fs::read_to_string(file)
                .with_context(|| format!("reading {}", file.display()))?;
            let proof: ProofFile =
                serde_json::from_str(&text).context("the file is not an Envelope Proof")?;
            finish(
                checks::envelope_proof(&api, &node, &proof, market.as_deref()).await?,
                req.json,
            )
        }
        Action::Thaw { k_root, alias } => {
            let key: [u8; 32] = hex::decode(k_root)
                .ok()
                .and_then(|b| b.try_into().ok())
                .context("k_root must be 32 bytes of hex")?;
            let out = checks::thaw(&api, &node, &key, alias.as_deref()).await?;
            let ok = out.checks.ok;
            let text = if req.json {
                serde_json::to_string_pretty(
                    &json!({"checks": out.checks, "thaw_report": out.thaw}),
                )?
            } else {
                format!(
                    "{}{}\n",
                    out.checks.render(),
                    serde_json::to_string_pretty(&out.thaw)?
                )
            };
            Ok((text, ok))
        }
    }
}

fn finish(report: Report, json: bool) -> Result<(String, bool)> {
    let ok = report.ok;
    Ok((
        if json {
            serde_json::to_string_pretty(&report)?
        } else {
            report.render()
        },
        ok,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }
    fn no_env(_: &str) -> Option<String> {
        None
    }

    #[test]
    fn parses_commands_flags_and_defaults() {
        let Invocation::Run(r) = parse(
            &args(&["all", "abc", "--node", "http://n", "--json"]),
            no_env,
        )
        .unwrap() else {
            panic!()
        };
        assert_eq!(r.action, Action::Market(Command::All, "abc".into()));
        assert_eq!(
            (r.node.as_str(), r.api.as_str(), r.json),
            ("http://n", DEFAULT_API, true)
        );
        let Invocation::Run(r) = parse(&args(&["pools", "abc"]), |k| {
            (k == "FROSTED_API_URL").then(|| "http://env".into())
        })
        .unwrap() else {
            panic!()
        };
        assert_eq!(
            (r.node.as_str(), r.api.as_str()),
            (DEFAULT_NODE, "http://env")
        );
        let Invocation::Run(r) = parse(&args(&["thaw", "00", "--alias", "orion"]), no_env).unwrap()
        else {
            panic!()
        };
        assert_eq!(
            r.action,
            Action::Thaw {
                k_root: "00".into(),
                alias: Some("orion".into())
            }
        );
        let Invocation::Run(r) = parse(
            &args(&["envelope-proof", "p.json", "--market", "m"]),
            no_env,
        )
        .unwrap() else {
            panic!()
        };
        assert_eq!(
            r.action,
            Action::EnvelopeProof {
                file: "p.json".into(),
                market: Some("m".into())
            }
        );
    }

    #[test]
    fn rejects_bad_usage() {
        for bad in [
            &[][..],
            &["all"],
            &["all", "a", "b"],
            &["nope", "a"],
            &["all", "a", "--bogus"],
            &["all", "a", "--node"],
        ] {
            assert!(parse(&args(bad), no_env).is_err(), "{bad:?}");
        }
        assert_eq!(parse(&args(&["--help"]), no_env).unwrap(), Invocation::Help);
        assert_eq!(
            parse(&args(&["all", "a", "-h"]), no_env).unwrap(),
            Invocation::Help
        );
    }

    #[test]
    fn help_documents_every_command_option_and_exit_code() {
        for word in [
            "market",
            "pools",
            "resolution",
            "all",
            "thaw",
            "envelope-proof",
            "--node",
            "--api",
            "--json",
            "--alias",
            "--market",
            "EXIT CODES",
        ] {
            assert!(HELP.contains(word), "{word}");
        }
    }
}

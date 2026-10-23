use anyhow::{Context, Result, bail};
use payout_coordinator::{
    config::local_bind,
    keys::dealer,
    secrets::{Secrets, write_private_new},
};
use serde_json::{Value, json};
use std::{
    env,
    path::{Path, PathBuf},
    time::Duration,
};

fn random_key() -> [u8; 32] {
    use std::io::Read;
    let mut bytes = [0; 32];
    std::fs::File::open("/dev/urandom")
        .expect("OS entropy")
        .read_exact(&mut bytes)
        .expect("OS entropy");
    bytes
}
fn initialize_secrets(dir: &Path) -> Result<Value> {
    std::fs::create_dir(dir)?;
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    write_private_new(&dir.join("secrets.key"), &random_key())?;
    let master = Secrets::load(&dir.join("secrets.key"))?;
    write_private_new(&dir.join("coordinator.key"), &random_key())?;
    for (name, file) in [
        ("envelope_sk", "envelope.sk"),
        ("hot_wallet_seed", "hot-wallet.seed"),
    ] {
        write_private_new(&dir.join(file), &master.seal(name, &random_key())?)?;
    }
    for i in 1..=3 {
        write_private_new(&dir.join(format!("member-{i}.key")), &random_key())?;
    }
    let coordinator = payout_coordinator::secrets::read_private(&dir.join("coordinator.key"))?;
    Ok(
        json!({"directory":dir,"coordinator_pubkey":payout_coordinator::coordinator_pubkey(coordinator.as_slice().try_into()?) }),
    )
}
async fn run() -> Result<Value> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.len() == 4 && args[..3] == ["secrets", "init", "--dir"] {
        return initialize_secrets(Path::new(&args[3]));
    }
    let bind =
        local_bind(&env::var("COORDINATOR_BIND").unwrap_or_else(|_| "127.0.0.1:8090".into()))?;
    let url = format!("http://{bind}");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(660))
        .redirect(reqwest::redirect::Policy::none())
        .build()?;
    let response = match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["committee", "init", rest @ ..] => {
            let mut dir = None;
            let mut keys = Vec::new();
            for pair in rest.chunks(2) {
                if pair.len() != 2 {
                    bail!("committee init --share-dir DIR --member-key KEY (three times)");
                }
                match pair[0] {
                    "--share-dir" => dir = Some(PathBuf::from(pair[1])),
                    "--member-key" => keys.push(PathBuf::from(pair[1])),
                    _ => bail!("unknown committee argument"),
                }
            }
            let dir = dir.context("--share-dir required")?;
            let committee = if dir.exists() {
                serde_json::from_slice(&std::fs::read(dir.join("committee.json"))?)?
            } else {
                dealer(&dir, &keys)?
            };
            client
                .post(format!("{url}/committee"))
                .json(&committee)
                .send()
                .await?
        }
        ["market", "create", "--file", file] => {
            let input: payout_coordinator::market::CreateMarket =
                serde_json::from_slice(&std::fs::read(file)?)?;
            client
                .post(format!("{url}/markets"))
                .json(&input)
                .send()
                .await?
        }
        ["market", "status", id] => client.get(format!("{url}/markets/{id}")).send().await?,
        ["payout", "status", id] => {
            client
                .get(format!("{url}/markets/{id}/payout"))
                .send()
                .await?
        }
        ["payout", "plan", id] => {
            client
                .post(format!("{url}/markets/{id}/payout"))
                .send()
                .await?
        }
        ["payout", "follow-up", id] => {
            client
                .post(format!("{url}/markets/{id}/payout/follow-up"))
                .send()
                .await?
        }
        ["payout", "sign", id, "--plan-hash", hash, rest @ ..] => {
            return sign_payout(&client, &url, id, hash, rest).await;
        }
        ["hot-wallet", "init"] => client.post(format!("{url}/hot-wallet")).send().await?,
        _ => bail!(
            "usage: frosted-operator secrets init --dir DIR | committee init --share-dir DIR --member-key KEY (three times) | market create --file FILE | market status ID | hot-wallet init"
        ),
    };
    let status = response.status();
    let body: Value = response.json().await?;
    if !status.is_success() {
        bail!("{}", serde_json::to_string(&body)?);
    }
    Ok(body)
}
#[tokio::main]
async fn main() {
    match run().await {
        Ok(value) => println!("{}", value),
        Err(error) => {
            let message = error.to_string();
            if let Ok(value) = serde_json::from_str::<Value>(&message) {
                eprintln!("{value}");
            } else {
                eprintln!("{}", json!({"error":{"code":"INTERNAL","message":message}}));
            }
            std::process::exit(1);
        }
    }
}

async fn sign_payout(
    client: &reqwest::Client,
    url: &str,
    id: &str,
    hash: &str,
    args: &[&str],
) -> Result<Value> {
    use payout_coordinator::{
        frost_sign::{self, SigningRequest},
        keys::Committee,
    };
    use std::io::{self, Write};
    if args.len() != 8
        || args[0] != "--share"
        || args[2] != "--member-key"
        || args[4] != "--share"
        || args[6] != "--member-key"
    {
        bail!(
            "payout sign ID --plan-hash HASH --share FILE --member-key FILE --share FILE --member-key FILE"
        )
    }
    let response = client
        .post(format!("{url}/markets/{id}/payout/prepare"))
        .json(&json!({"plan_hash":hash}))
        .send()
        .await?;
    if !response.status().is_success() {
        bail!("payout preparation failed HTTP {}", response.status())
    }
    let prepared: Value = response.json().await?;
    if prepared["public"]["plan_hash"] != hash {
        bail!("PLAN_HASH_MISMATCH")
    }
    let committee: Committee = serde_json::from_value(prepared["committee"].clone())?;
    let mut members = Vec::new();
    for pair in [(&args[1], &args[3]), (&args[5], &args[7])] {
        eprintln!("Plan {hash}\n{}", prepared["public"]["summary"]);
        eprint!(
            "Member share {}: type the complete plan_hash to approve: ",
            pair.0
        );
        io::stderr().flush()?;
        let timeout = env::var("SIGNING_SESSION_TIMEOUT_SECS")
            .ok()
            .map(|v| v.parse::<u64>())
            .transpose()?
            .unwrap_or(payout_coordinator::config::SIGNING_SESSION_TIMEOUT_SECS);
        if timeout == 0 {
            bail!("signing timeout must be positive")
        }
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut line = String::new();
            let result = io::stdin().read_line(&mut line).map(|_| line);
            let _ = sender.send(result);
        });
        // The detached stdin thread cannot hold up runtime shutdown after the bounded wait.
        let answer = tokio::task::spawn_blocking(move || {
            receiver.recv_timeout(Duration::from_secs(timeout))
        })
        .await?
        .context("signing approval timed out")??;
        if answer.trim() != hash {
            bail!("member declined plan")
        }
        members.push(frost_sign::load_member(
            Path::new(pair.0),
            Path::new(pair.1),
            &committee,
        )?);
    }
    let mut signatures = Vec::new();
    for batch in prepared["batches"]
        .as_array()
        .context("missing signing batches")?
    {
        let requests: Vec<SigningRequest> =
            serde_json::from_value(batch["signing_requests"].clone())?;
        signatures.push(frost_sign::sign(&committee, &members, &requests)?);
    }
    let response = client
        .post(format!("{url}/markets/{id}/payout/submit"))
        .json(&json!({"plan_hash":hash,"signatures":signatures}))
        .send()
        .await?;
    if !response.status().is_success() {
        bail!("payout submission failed HTTP {}", response.status())
    }
    Ok(response.json().await?)
}

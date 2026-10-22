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

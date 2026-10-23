use anyhow::{Result, anyhow, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use rand::{rand_core::UnwrapErr, rngs::SysRng};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    num::{NonZeroU32, NonZeroUsize},
    str::FromStr,
    time::Duration,
};
use tonic::transport::Channel;
use zcash_client_backend::{
    data_api::{
        Account, AccountBirthday, AccountPurpose, WalletRead, WalletWrite,
        chain::ChainState,
        wallet::{
            ConfirmationsPolicy, SpendingKeys, create_proposed_transactions,
            input_selection::{GreedyInputSelector, SpendPolicy},
            propose_transfer,
        },
    },
    fees::{DustOutputPolicy, SplitPolicy, StandardFeeRule, standard::MultiOutputChangeStrategy},
    proto::service::{self, compact_tx_streamer_client::CompactTxStreamerClient},
    wallet::OvkPolicy,
};
use zcash_client_sqlite::{WalletDb, util::SystemClock, wallet::init::init_wallet_db};
use zcash_keys::keys::{UnifiedAddressRequest, UnifiedFullViewingKey, UnifiedSpendingKey};
use zcash_protocol::{
    ShieldedPool,
    consensus::{BlockHeight, NetworkType, NetworkUpgrade, Parameters},
    local_consensus::LocalNetwork,
    memo::{Memo, MemoBytes},
    value::Zatoshis,
};
mod cache;
mod payout;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    op: String,
    network: String,
    rpc_url: String,
    zaino_url: String,
    birthday_height: u32,
    ufvk: Option<String>,
    seed_hex: Option<String>,
    snapshot_b64: Option<String>,
    address: Option<String>,
    amount_zat: Option<u64>,
    memo: Option<String>,
    tx_hex: Option<String>,
    #[serde(default)]
    inputs: Vec<payout::Input>,
    #[serde(default)]
    outputs: Vec<payout::Output>,
    fee_zat: Option<u64>,
    max_actions: Option<u32>,
    pczt_b64: Option<String>,
    #[serde(default)]
    signatures: Vec<payout::SpendSignature>,
}
#[derive(Serialize)]
struct Response {
    snapshot_b64: String,
    balance_zat: u64,
    address: Option<String>,
    txid: Option<String>,
    account_uuid: String,
    tx_hex: Option<String>,
    #[serde(flatten)]
    payout: payout::ResultFields,
}
#[derive(Clone, Copy)]
struct Network {
    kind: NetworkType,
    heights: LocalNetwork,
}
impl Parameters for Network {
    fn network_type(&self) -> NetworkType {
        self.kind
    }
    fn activation_height(&self, n: NetworkUpgrade) -> Option<BlockHeight> {
        self.heights.activation_height(n)
    }
}
fn matches_chain(kind: NetworkType, chain: &str) -> bool {
    match kind {
        NetworkType::Main => matches!(chain, "main" | "mainnet"),
        NetworkType::Test => matches!(chain, "test" | "testnet"),
        // Zebra reports "test" for its custom regtest network. Heights still come from RPC.
        NetworkType::Regtest => matches!(chain, "regtest" | "test"),
    }
}
async fn network(r: &Request) -> Result<Network> {
    let kind = match r.network.as_str() {
        "mainnet" => NetworkType::Main,
        "testnet" => NetworkType::Test,
        "regtest" => NetworkType::Regtest,
        _ => return Err(anyhow!("invalid network")),
    };
    let v: serde_json::Value = reqwest::Client::new()
        .post(&r.rpc_url)
        .timeout(Duration::from_secs(15))
        .json(&serde_json::json!({"jsonrpc":"2.0","id":1,"method":"getblockchaininfo","params":[]}))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    ensure!(v["error"].is_null(), "node RPC failed");
    let chain = v["result"]["chain"]
        .as_str()
        .ok_or(anyhow!("missing network"))?;
    ensure!(matches_chain(kind, chain), "node network mismatch");
    let upgrades = v["result"]["upgrades"]
        .as_object()
        .ok_or(anyhow!("missing upgrades"))?;
    let h = |name: &str| {
        upgrades
            .values()
            .find(|u| u["name"] == name)
            .and_then(|u| u["activationheight"].as_u64())
            .and_then(|h| u32::try_from(h).ok())
            .map(BlockHeight::from_u32)
    };
    Ok(Network {
        kind,
        heights: LocalNetwork {
            overwinter: h("Overwinter"),
            sapling: h("Sapling"),
            blossom: h("Blossom"),
            heartwood: h("Heartwood"),
            canopy: h("Canopy"),
            nu5: h("NU5"),
            nu6: h("NU6"),
            nu6_1: h("NU6.1"),
            nu6_2: h("NU6.2"),
            nu6_3: h("NU6.3"),
            nu7: h("NU7"),
        },
    })
}
async fn birthday(
    client: &mut CompactTxStreamerClient<Channel>,
    p: Network,
    height: u32,
) -> Result<AccountBirthday> {
    let sapling = p
        .activation_height(NetworkUpgrade::Sapling)
        .ok_or(anyhow!("Sapling inactive"))?;
    let height = BlockHeight::from_u32(height).max(sapling);
    if height == sapling {
        let b = client
            .get_block(service::BlockId {
                height: u64::from(height),
                ..Default::default()
            })
            .await?
            .into_inner();
        Ok(AccountBirthday::from_parts(
            ChainState::empty(height.saturating_sub(1), b.prev_hash()),
            None,
        ))
    } else {
        Ok(AccountBirthday::from_treestate(
            client
                .get_tree_state(service::BlockId {
                    height: u64::from(height) - 1,
                    ..Default::default()
                })
                .await?
                .into_inner(),
            None,
        )?)
    }
}
fn connection(snapshot: Option<&str>) -> Result<Connection> {
    let mut c = Connection::open_in_memory()?;
    if let Some(s) = snapshot {
        let bytes = STANDARD.decode(s)?;
        ensure!(bytes.len() <= 128 * 1024 * 1024, "snapshot too large");
        c.deserialize_read_exact("main", bytes.as_slice(), bytes.len(), false)?;
    }
    rusqlite::vtab::array::load_module(&c)?;
    c.execute_batch("PRAGMA temp_store=MEMORY; PRAGMA journal_mode=MEMORY;")?;
    Ok(c)
}
async fn run(r: Request) -> Result<Response> {
    ensure!(
        [
            "import",
            "hot_init",
            "sync",
            "announce",
            "broadcast",
            "payout_build",
            "payout_extract"
        ]
        .contains(&r.op.as_str()),
        "invalid operation"
    );
    let p = network(&r).await?;
    let mut endpoint = tonic::transport::Endpoint::from_shared(r.zaino_url.clone())?
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(120));
    // Public lightwalletd servers (testnet.zec.rocks) are HTTPS only; local Zaino stays plaintext.
    if r.zaino_url.starts_with("https://") {
        endpoint = endpoint.tls_config(tonic::transport::ClientTlsConfig::new().with_webpki_roots())?;
    }
    let mut client = CompactTxStreamerClient::new(endpoint.connect().await?);
    if r.op == "broadcast" {
        let raw = hex::decode(r.tx_hex.as_deref().ok_or(anyhow!("missing transaction"))?)?;
        ensure!(raw.len() <= 2_000_000, "transaction too large");
        let parsed = zcash_primitives::transaction::Transaction::read(
            raw.as_slice(),
            zcash_protocol::consensus::BranchId::Nu6_3,
        )?;
        let txid = parsed.txid().to_string();
        let result = client
            .send_transaction(service::RawTransaction {
                data: raw,
                ..Default::default()
            })
            .await?
            .into_inner();
        ensure!(result.error_code == 0, "transaction rejected");
        return Ok(Response {
            snapshot_b64: r.snapshot_b64.unwrap_or_default(),
            balance_zat: 0,
            address: None,
            txid: Some(txid),
            account_uuid: String::new(),
            tx_hex: None,
            payout: Default::default(),
        });
    }
    let mut conn = connection(r.snapshot_b64.as_deref())?;
    let mut db = WalletDb::from_connection(&mut conn, p, SystemClock, UnwrapErr(SysRng));
    init_wallet_db(&mut db, None)?;
    let seed = r
        .seed_hex
        .as_deref()
        .map(hex::decode)
        .transpose()?
        .map(secrecy::SecretVec::new);
    if r.op == "import" || r.op == "hot_init" {
        ensure!(r.snapshot_b64.is_none(), "wallet already exists");
        let b = birthday(&mut client, p, r.birthday_height).await?;
        if r.op == "import" {
            let ufvk = UnifiedFullViewingKey::decode(
                &p,
                r.ufvk.as_deref().ok_or(anyhow!("missing ufvk"))?,
            )?;
            db.import_account_ufvk("escrow", &ufvk, &b, AccountPurpose::ViewOnly, None)?;
        } else {
            db.create_account(
                "hot",
                seed.as_ref().ok_or(anyhow!("missing seed"))?,
                &b,
                None,
            )?;
        }
    }
    if r.op == "sync" || r.op == "announce" || r.op == "payout_build" {
        zcash_client_backend::sync::run(&mut client, &p, &cache::Cache::default(), &mut db, 1000)
            .await?;
    }
    let ids = db.get_account_ids()?;
    ensure!(ids.len() == 1, "expected one account");
    let id = ids[0];
    let account = db.get_account(id)?.ok_or(anyhow!("missing account"))?;
    let address = if r.op == "import" || r.op == "hot_init" {
        db.get_next_available_address(id, UnifiedAddressRequest::ORCHARD)?
            .map(|a| a.0.encode(&p))
    } else {
        None
    };
    let mut txid = None;
    let mut tx_hex = None;
    if r.op == "announce" {
        use secrecy::ExposeSecret;
        let usk = UnifiedSpendingKey::from_seed(
            &p,
            seed.as_ref()
                .ok_or(anyhow!("missing seed"))?
                .expose_secret(),
            account
                .source()
                .key_derivation()
                .ok_or(anyhow!("view-only account"))?
                .account_index(),
        )?;
        let memo = Memo::from_str(r.memo.as_deref().ok_or(anyhow!("missing memo"))?)?;
        let payment = zip321::Payment::new(
            zcash_address::ZcashAddress::from_str(
                r.address.as_deref().ok_or(anyhow!("missing address"))?,
            )?,
            Some(Zatoshis::from_u64(
                r.amount_zat.ok_or(anyhow!("missing amount"))?,
            )?),
            Some(MemoBytes::from(memo)),
            None,
            None,
            vec![],
        )
        .map_err(|_| anyhow!("invalid payment"))?;
        let change = MultiOutputChangeStrategy::new(
            StandardFeeRule::Zip317,
            None,
            ShieldedPool::Orchard,
            DustOutputPolicy::default(),
            SplitPolicy::with_min_output_value(
                NonZeroUsize::new(1).unwrap(),
                Zatoshis::from_u64(10_000_000)?,
            ),
        );
        let policy = if r.network == "regtest" {
            ConfirmationsPolicy::new_symmetrical(NonZeroU32::new(1).unwrap(), false)
        } else {
            ConfirmationsPolicy::default()
        };
        let proposal =
            propose_transfer::<_, _, _, _, zcash_client_sqlite::error::SqliteClientError>(
                &mut db,
                &p,
                id,
                &GreedyInputSelector::new(),
                &change,
                zip321::TransactionRequest::new(vec![payment])?,
                policy,
                &SpendPolicy::default(),
                None,
                None,
            )
            .map_err(|_| anyhow!("cannot propose transfer"))?;
        let prover = zcash_proofs::prover::LocalTxProver::bundled();
        let txids = create_proposed_transactions::<
            _,
            _,
            std::convert::Infallible,
            _,
            std::convert::Infallible,
            _,
        >(
            &mut db,
            &p,
            &SystemClock,
            &mut UnwrapErr(SysRng),
            &prover,
            &prover,
            &SpendingKeys::from_unified_spending_key(usk),
            OvkPolicy::Sender,
            &proposal,
            None,
        )?;
        ensure!(txids.len() == 1, "expected one transaction");
        let id = *txids.first();
        let tx = db
            .get_transaction(id)?
            .ok_or(anyhow!("missing transaction"))?;
        let mut raw = service::RawTransaction::default();
        tx.write(&mut raw.data)?;
        tx_hex = Some(hex::encode(&raw.data));
        txid = Some(id.to_string());
    }
    let mut payout = payout::ResultFields::default();
    if r.op == "payout_build" {
        payout = payout::build(&r, &p, &mut db, id)?;
    } else if r.op == "payout_extract" {
        let (raw, id) = payout::extract(&r, &mut db)?;
        tx_hex = Some(raw);
        txid = Some(id);
    }
    let balance_zat = db
        .get_wallet_summary(ConfirmationsPolicy::MIN)?
        .map(|s| {
            s.account_balances()
                .values()
                .map(|b| u64::from(b.total()))
                .sum()
        })
        .unwrap_or(0);
    let snapshot_b64 = STANDARD.encode(&*conn.serialize("main")?);
    Ok(Response {
        snapshot_b64,
        balance_zat,
        address,
        txid,
        account_uuid: id.expose_uuid().to_string(),
        tx_hex,
        payout,
    })
}
#[tokio::main]
async fn main() {
    // Inputs and snapshots contain secrets; never include them in errors or process arguments.
    let result = async {
        let mut input = String::new();
        std::io::stdin()
            .take(180 * 1024 * 1024)
            .read_to_string(&mut input)?;
        let r: Request = serde_json::from_str(&input)?;
        run(r).await
    }
    .await;
    match result {
        Ok(r) => {
            if serde_json::to_writer(std::io::stdout().lock(), &r).is_err() {
                std::process::exit(1)
            }
        }
        Err(_) => {
            let _ = std::io::stderr().write_all(b"wallet operation failed\n");
            std::process::exit(1)
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn zebra_regtest_reports_test_but_mainnet_must_match() {
        assert!(matches_chain(NetworkType::Regtest, "test"));
        assert!(matches_chain(NetworkType::Regtest, "regtest"));
        assert!(!matches_chain(NetworkType::Regtest, "main"));
        assert!(!matches_chain(NetworkType::Main, "test"));
    }
    #[test]
    fn snapshot_is_memory_only_and_round_trips() {
        let c = connection(None).unwrap();
        c.execute_batch("CREATE TABLE secret(x TEXT); INSERT INTO secret VALUES ('secret-ufvk');")
            .unwrap();
        let snapshot = STANDARD.encode(&*c.serialize("main").unwrap());
        let c = connection(Some(&snapshot)).unwrap();
        assert_eq!(
            c.query_row("SELECT x FROM secret", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "secret-ufvk"
        );
        let path: String = c
            .query_row(
                "SELECT file FROM pragma_database_list WHERE name='main'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(path, "");
        assert_eq!(
            c.query_row("PRAGMA temp_store", [], |r| r.get::<_, i32>(0))
                .unwrap(),
            2
        );
    }
}

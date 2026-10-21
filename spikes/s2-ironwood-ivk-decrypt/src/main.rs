//! M0.2: decrypt an Ironwood note sent to an Orchard-only UA, using only the IVK.
//!
//!   s2 address <index>        print the Orchard-only regtest UA at diversifier index
//!   s2 decrypt <tx-hex-file>  trial-decrypt every Ironwood action with the IVK
use std::{env, fs};

use anyhow::{Context, Result, anyhow, bail};
use orchard::keys::{FullViewingKey, Scope, SpendingKey};
use orchard::note_encryption::IronwoodDomain;
use zcash_keys::address::UnifiedAddress;
use zcash_note_encryption::try_note_decryption;
use zcash_primitives::transaction::Transaction;
use zcash_protocol::consensus::{BlockHeight, BranchId};
use zcash_protocol::local_consensus::LocalNetwork;

/// Must match infra/zebra/regtest.toml.
fn regtest() -> LocalNetwork {
    let h = |n| Some(BlockHeight::from_u32(n));
    LocalNetwork {
        overwinter: h(1), sapling: h(1), blossom: h(1), heartwood: h(1), canopy: h(1),
        nu5: h(1), nu6: h(1), nu6_1: h(1), nu6_2: h(1), nu6_3: h(10),
    }
}

/// Deterministic spike key (never use for real funds).
fn fvk() -> FullViewingKey {
    let sk = SpendingKey::from_bytes([0x2a; 32]).expect("valid spike key");
    FullViewingKey::from(&sk)
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("address") => {
            let j: u32 = args.get(2).context("index")?.parse()?;
            let addr = fvk().address_at(j, Scope::External);
            let ua = UnifiedAddress::from_receivers(Some(addr), None)
                .ok_or_else(|| anyhow!("UA"))?;
            println!("{}", ua.encode(&regtest()));
        }
        Some("address-test") => {
            // Testnet UA for the M0.6 Zodl memo spike; the receiver is decryptable by `decrypt`.
            let j: u32 = args.get(2).context("index")?.parse()?;
            let addr = fvk().address_at(j, Scope::External);
            let ua = UnifiedAddress::from_receivers(Some(addr), None).ok_or_else(|| anyhow!("UA"))?;
            println!("{}", ua.encode(&zcash_protocol::consensus::Network::TestNetwork));
        }
        Some("decrypt") => {
            let hex_tx = fs::read_to_string(args.get(2).context("tx hex file")?)?;
            let bytes = hex::decode(hex_tx.trim())?;
            let tx = Transaction::read(&bytes[..], BranchId::Nu6_3)?; // branch id is also read from the v6 header
            println!("txid {} version {:?}", tx.txid(), tx.version());
            let bundle = tx.ironwood_bundle().ok_or_else(|| anyhow!("no Ironwood bundle"))?;
            let ivk = fvk().to_ivk(Scope::External);
            let pivk = ivk.prepare();
            let mut found = 0;
            for (i, action) in bundle.actions().iter().enumerate() {
                let domain = IronwoodDomain::for_action(action);
                if let Some((note, recipient, memo)) = try_note_decryption(&domain, &pivk, action) {
                    found += 1;
                    let j = ivk.diversifier_index(&recipient).map(|d| {
                        let b = *d.as_bytes();
                        u64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]])
                    });
                    let end = memo.iter().rposition(|b| *b != 0).map_or(0, |p| p + 1);
                    println!(
                        "action {i}: value={} zat diversifier_index={:?} memo={:?}",
                        note.value().inner(),
                        j,
                        String::from_utf8_lossy(&memo[..end])
                    );
                }
            }
            if found == 0 {
                bail!("no action decrypted with the IVK");
            }
        }
        _ => bail!("usage: s2 address <index> | s2 decrypt <tx-hex-file>"),
    }
    Ok(())
}

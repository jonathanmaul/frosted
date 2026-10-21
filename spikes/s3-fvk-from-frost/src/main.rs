//! M0.3: Glass Escrow keys from a FROST RedPallas group key.
//!
//!   s3 dealer <out.json>              trusted dealer 2-of-3 → ak; random nk, rivk → FVK → UFVK/UIVK
//!   s3 decrypt <out.json> <tx.hex>    decrypt Ironwood actions using only the published UIVK
use std::{collections::BTreeMap, env, fs};

use anyhow::{Context, Result, anyhow, bail};
use orchard::keys::{FullViewingKey, Scope};
use orchard::note_encryption::IronwoodDomain;
use pasta_curves::{group::ff::{Field, PrimeField}, pallas};
use rand_core::OsRng;
use reddsa::frost::redpallas::{self as frost, keys::IdentifierList};
use zcash_keys::address::UnifiedAddress;
use zcash_keys::keys::{UnifiedFullViewingKey, UnifiedIncomingViewingKey};
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

fn dealer(out: &str) -> Result<()> {
    let mut rng = OsRng;
    // generate_with_dealer applies EvenY, so the group key is a valid Orchard `ak` (ỹ = 0).
    let (shares, pubkeys) =
        frost::keys::generate_with_dealer(3, 2, IdentifierList::Default, &mut rng)?;
    let ak: Vec<u8> = pubkeys.verifying_key().serialize()?;
    let nk = pallas::Base::random(&mut rng).to_repr();
    let rivk = pallas::Scalar::random(&mut rng).to_repr();

    let mut raw = [0u8; 96];
    raw[..32].copy_from_slice(&ak);
    raw[32..64].copy_from_slice(&nk);
    raw[64..].copy_from_slice(&rivk);
    let fvk = FullViewingKey::from_bytes(&raw)
        .ok_or_else(|| anyhow!("FVK from ak||nk||rivk rejected"))?;

    let net = regtest();
    let ufvk = UnifiedFullViewingKey::from_orchard_fvk(fvk.clone())
        .map_err(|e| anyhow!("UFVK: {e:?}"))?;
    let uivk = ufvk.to_unified_incoming_viewing_key();
    let uivk_str = uivk.encode(&net);

    // Outcome Addresses: diversifier index j = outcome_index, derived from the UIVK alone.
    let parsed = UnifiedIncomingViewingKey::decode(&net, &uivk_str).map_err(|e| anyhow!(e))?;
    let ivk = parsed.orchard().clone().context("UIVK has no Orchard item")?;
    let mut addresses = Vec::new();
    for j in 0u32..3 {
        let from_ivk = ivk.address_at(j);
        assert_eq!(from_ivk, fvk.address_at(j, Scope::External), "UIVK/FVK address mismatch");
        let ua = UnifiedAddress::from_receivers(Some(from_ivk), None).context("UA")?;
        addresses.push(ua.encode(&net));
    }

    let key_packages: BTreeMap<String, frost::keys::KeyPackage> = shares
        .into_iter()
        .map(|(id, s)| Ok((hex::encode(id.serialize()), frost::keys::KeyPackage::try_from(s)?)))
        .collect::<Result<_>>()?;

    let doc = serde_json::json!({
        "ak": hex::encode(&ak), "nk": hex::encode(nk), "rivk": hex::encode(rivk),
        "ufvk": ufvk.encode(&net), "uivk": uivk_str, "outcome_addresses": addresses,
        "public_key_package": pubkeys, "key_packages": key_packages,
    });
    fs::write(out, serde_json::to_string_pretty(&doc)?)?;
    println!("ak   {}", hex::encode(&ak));
    println!("UFVK {}", ufvk.encode(&net));
    println!("UIVK {}", doc["uivk"].as_str().unwrap());
    for (j, a) in addresses.iter().enumerate() {
        println!("outcome {j} {a}");
    }
    Ok(())
}

fn decrypt(out: &str, tx_hex: &str) -> Result<()> {
    let doc: serde_json::Value = serde_json::from_str(&fs::read_to_string(out)?)?;
    let uivk = UnifiedIncomingViewingKey::decode(&regtest(), doc["uivk"].as_str().context("uivk")?)
        .map_err(|e| anyhow!(e))?;
    let ivk = uivk.orchard().clone().context("UIVK has no Orchard item")?;
    let pivk = ivk.prepare();
    let bytes = hex::decode(fs::read_to_string(tx_hex)?.trim())?;
    let tx = Transaction::read(&bytes[..], BranchId::Nu6_3)?;
    let bundle = tx.ironwood_bundle().context("no Ironwood bundle")?;
    let mut found = 0;
    for (i, a) in bundle.actions().iter().enumerate() {
        if let Some((note, recipient, memo)) =
            try_note_decryption(&IronwoodDomain::for_action(a), &pivk, a)
        {
            found += 1;
            let j = ivk.diversifier_index(&recipient).map(|d| d.as_bytes()[0]);
            let end = memo.iter().rposition(|b| *b != 0).map_or(0, |p| p + 1);
            println!("action {i}: value={} zat outcome_index={j:?} memo={:?}",
                note.value().inner(), String::from_utf8_lossy(&memo[..end]));
        }
    }
    if found == 0 {
        bail!("nothing decrypted with the UIVK");
    }
    Ok(())
}

fn main() -> Result<()> {
    let a: Vec<String> = env::args().collect();
    match (a.get(1).map(String::as_str), a.get(2), a.get(3)) {
        (Some("dealer"), Some(out), _) => dealer(out),
        (Some("decrypt"), Some(out), Some(tx)) => decrypt(out, tx),
        _ => bail!("usage: s3 dealer <out.json> | s3 decrypt <out.json> <tx.hex>"),
    }
}

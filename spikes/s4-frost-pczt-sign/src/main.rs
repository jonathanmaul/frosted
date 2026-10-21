//! M0.4: authorize Ironwood spends of a FROST-controlled escrow inside a PCZT.
//!
//!   s4 sign <escrow.json> <in.pczt> <out.pczt>
//!
//! For every Ironwood action: dummy spends are signed with the PCZT's `dummy_sk`; real
//! escrow spends get a 2-of-3 FROST RedPallas signature over the shielded sighash, using
//! the PCZT's `alpha` as the re-randomizer, and are applied with `apply_ironwood_signature`
//! (which verifies against the action's `rk`).
#![allow(deprecated)] // frost_rerandomized::sign is the only public external-randomizer path in v3.

use std::{collections::BTreeMap, env, fs, time::Instant};

use anyhow::{Context, Result, anyhow, bail};
use pczt::{Pczt, roles::{low_level_signer, signer::Signer}};
use reddsa::frost::redpallas::{self as frost, PallasBlake2b512};

struct Info {
    alpha: Option<[u8; 32]>,
    dummy_sk: Option<orchard::keys::SpendingKey>,
    /// Dummy spends are already signed by the IO Finalizer (which then clears dummy_sk).
    already_signed: bool,
}

#[derive(Debug)]
struct E(String);
impl From<pczt::roles::low_level_signer::OrchardParseError> for E {
    fn from(e: pczt::roles::low_level_signer::OrchardParseError) -> Self { E(format!("{e:?}")) }
}

fn main() -> Result<()> {
    let a: Vec<String> = env::args().collect();
    let [_, cmd, escrow, input, output] = a.as_slice() else {
        bail!("usage: s4 sign <escrow.json> <in.pczt> <out.pczt>");
    };
    if cmd != "sign" { bail!("unknown command {cmd}"); }

    let doc: serde_json::Value = serde_json::from_str(&fs::read_to_string(escrow)?)?;
    let pubkeys: frost::keys::PublicKeyPackage = serde_json::from_value(doc["public_key_package"].clone())?;
    let key_packages: BTreeMap<String, frost::keys::KeyPackage> =
        serde_json::from_value(doc["key_packages"].clone())?;
    // Two of the three members sign.
    let signers: Vec<&frost::keys::KeyPackage> = key_packages.values().take(2).collect();

    let pczt = Pczt::parse(&fs::read(input)?).map_err(|e| anyhow!("parse PCZT: {e:?}"))?;

    // Read alpha / dummy_sk per Ironwood action (read-only use of the low-level signer).
    let mut infos: Vec<Info> = Vec::new();
    low_level_signer::Signer::new(pczt.clone())
        .sign_ironwood_with(|_, bundle, _| -> Result<(), E> {
            for action in bundle.actions() {
                infos.push(Info {
                    alpha: action.spend().alpha().map(|s| {
                        use ff::PrimeField as _;
                        s.to_repr()
                    }),
                    dummy_sk: action.spend().dummy_sk().clone(),
                    already_signed: action.spend().spend_auth_sig().is_some(),
                });
            }
            Ok(())
        })
        .map_err(|e| anyhow!("{e:?}"))?;

    let mut signer = Signer::new(pczt).map_err(|e| anyhow!("signer: {e:?}"))?;
    let sighash = signer.shielded_sighash();
    println!("shielded sighash {}", hex::encode(sighash));

    let mut sessions = 0;
    for (i, info) in infos.iter().enumerate() {
        if info.already_signed {
            println!("action {i}: already signed by the IO Finalizer (dummy spend)");
            continue;
        }
        if let Some(sk) = &info.dummy_sk {
            let ask = orchard::keys::SpendAuthorizingKey::from(sk);
            signer.sign_ironwood(rand::rng(), i, &ask).map_err(|e| anyhow!("dummy {i}: {e:?}"))?;
            println!("action {i}: dummy spend signed with dummy_sk");
            continue;
        }
        let alpha = info.alpha.context("missing alpha")?;
        let start = Instant::now();
        let randomizer = frost_rerandomized::Randomizer::<PallasBlake2b512>::deserialize(&alpha)
            .map_err(|e| anyhow!("alpha as randomizer: {e:?}"))?;
        let params = frost_rerandomized::RandomizedParams::from_randomizer(pubkeys.verifying_key(), randomizer);

        // Round 1: each signer commits.
        let mut rng = rand_core_06::OsRng;
        let mut nonces = BTreeMap::new();
        let mut commitments = BTreeMap::new();
        for kp in &signers {
            let (n, c) = frost::round1::commit(kp.signing_share(), &mut rng);
            nonces.insert(*kp.identifier(), n);
            commitments.insert(*kp.identifier(), c);
        }
        let package = frost::SigningPackage::new(commitments, &sighash);
        // Round 2: each signer produces a share with the PCZT's randomizer.
        let mut shares = BTreeMap::new();
        for kp in &signers {
            let share = frost_rerandomized::sign(&package, &nonces[kp.identifier()], kp, randomizer)
                .map_err(|e| anyhow!("round2: {e:?}"))?;
            shares.insert(*kp.identifier(), share);
        }
        let sig = frost::rerandomized::aggregate(&package, &shares, &pubkeys, &params)
            .map_err(|e| anyhow!("aggregate: {e:?}"))?;
        let bytes: [u8; 64] = sig.serialize()?.try_into().map_err(|_| anyhow!("sig len"))?;
        signer
            .apply_ironwood_signature(i, orchard::primitives::redpallas::Signature::from(bytes))
            .map_err(|e| anyhow!("apply {i}: {e:?}"))?;
        sessions += 1;
        println!("action {i}: FROST 2-of-3 signature applied ({} ms)", start.elapsed().as_millis());
    }
    let out = signer.finish().serialize().map_err(|e| anyhow!("serialize: {e:?}"))?;
    fs::write(output, out)?;
    println!("FROST signing sessions: {sessions}");
    Ok(())
}

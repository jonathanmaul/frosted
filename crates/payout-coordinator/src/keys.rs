//! Stable derivation group; only encoded UFVK/UIVK crosses the wallet-helper boundary.
use crate::secrets::{Secrets, write_private_new};
use anyhow::{Context, Result, bail};
use orchard::keys::FullViewingKey;
use pasta_curves::{
    group::ff::{Field, PrimeField},
    pallas,
};
use rand_core::OsRng;
use reddsa::frost::redpallas::{self as frost, keys::IdentifierList};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use zcash_keys::{address::UnifiedAddress, keys::UnifiedFullViewingKey};
use zcash_protocol::{
    consensus::{BlockHeight, MAIN_NETWORK, Parameters, TEST_NETWORK},
    local_consensus::LocalNetwork,
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Committee {
    pub group_id: String,
    pub trusted_dealer: bool,
    pub public_key_package: frost::keys::PublicKeyPackage,
}

pub fn dealer(share_dir: &Path, member_keys: &[PathBuf]) -> Result<Committee> {
    if member_keys.len() != 3 {
        bail!("exactly three separate member keys are required");
    }
    let keys = member_keys
        .iter()
        .map(|p| Secrets::load(p))
        .collect::<Result<Vec<_>>>()?;
    let canonical = member_keys
        .iter()
        .map(|p| p.canonicalize())
        .collect::<Result<Vec<_>, _>>()?;
    if canonical
        .iter()
        .enumerate()
        .any(|(i, p)| canonical[..i].contains(p))
    {
        bail!("member keys must be distinct files");
    }
    let raw = member_keys
        .iter()
        .map(std::fs::read)
        .collect::<Result<Vec<_>, _>>()?;
    if raw.iter().enumerate().any(|(i, p)| raw[..i].contains(p)) {
        bail!("member keys must have distinct key material");
    }
    let (shares, public_key_package) =
        frost::keys::generate_with_dealer(3, 2, IdentifierList::Default, OsRng)?;
    let group_id = hex::encode(public_key_package.verifying_key().serialize()?);
    std::fs::create_dir(share_dir)?;
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(share_dir, std::fs::Permissions::from_mode(0o700))?;
    for ((id, share), key) in shares.into_iter().zip(&keys) {
        let package = frost::keys::KeyPackage::try_from(share)?;
        let id = hex::encode(id.serialize());
        let secret = zeroize::Zeroizing::new(serde_json::to_vec(&package)?);
        write_private_new(
            &share_dir.join(format!("{id}.share.enc")),
            &key.seal(&format!("frost/{group_id}/{id}"), &secret)?,
        )?;
    }
    let committee = Committee {
        group_id,
        trusted_dealer: true,
        public_key_package,
    };
    write_private_new(
        &share_dir.join("committee.json"),
        &serde_json::to_vec_pretty(&committee)?,
    )?;
    Ok(committee)
}

pub struct EscrowKeys {
    pub nk: [u8; 32],
    pub rivk: [u8; 32],
    pub ufvk: String,
    pub uivk: String,
    pub addresses: Vec<String>,
}

pub fn derive(committee: &Committee, network: &str, outcomes: usize) -> Result<EscrowKeys> {
    match network {
        "mainnet" => derive_for(committee, &MAIN_NETWORK, outcomes),
        "testnet" => derive_for(committee, &TEST_NETWORK, outcomes),
        "regtest" => derive_for(
            committee,
            &LocalNetwork {
                overwinter: None,
                sapling: None,
                blossom: None,
                heartwood: None,
                canopy: None,
                nu5: None,
                nu6: None,
                nu6_1: None,
                nu6_2: None,
                nu6_3: Some(BlockHeight::from_u32(1)),
                nu7: None,
            },
            outcomes,
        ),
        _ => bail!("invalid network"),
    }
}
fn derive_for<P: Parameters>(
    committee: &Committee,
    net: &P,
    outcomes: usize,
) -> Result<EscrowKeys> {
    if !(2..=4).contains(&outcomes) {
        bail!("2 to 4 outcomes required");
    }
    let ak = committee.public_key_package.verifying_key().serialize()?;
    let nk = pallas::Base::random(OsRng).to_repr();
    let rivk = pallas::Scalar::random(OsRng).to_repr();
    let mut raw = zeroize::Zeroizing::new([0; 96]);
    raw[..32].copy_from_slice(&ak);
    raw[32..64].copy_from_slice(&nk);
    raw[64..].copy_from_slice(&rivk);
    let fvk = FullViewingKey::from_bytes(&raw).context("invalid FROST-derived FVK")?;
    let ufvk = UnifiedFullViewingKey::from_orchard_fvk(fvk)
        .map_err(|_| anyhow::anyhow!("invalid UFVK"))?;
    let uivk = ufvk.to_unified_incoming_viewing_key();
    let ivk = uivk.orchard().as_ref().context("missing Orchard IVK")?;
    let addresses = (0..outcomes)
        .map(|i| {
            UnifiedAddress::from_receivers(Some(ivk.address_at(i as u32)), None, None, None, None)
                .context("invalid outcome address")
                .map(|a| a.encode(net))
        })
        .collect::<Result<_>>()?;
    Ok(EscrowKeys {
        nk,
        rivk,
        ufvk: ufvk
            .encode(net)
            .map_err(|_| anyhow::anyhow!("invalid UFVK"))?,
        uivk: uivk
            .encode(net)
            .map_err(|_| anyhow::anyhow!("invalid UIVK"))?,
        addresses,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dealer_and_fresh_per_market_keys() {
        let d = tempfile::tempdir().unwrap();
        let paths = (1..=3)
            .map(|i| {
                let p = d.path().join(format!("key{i}"));
                write_private_new(&p, &[i; 32]).unwrap();
                p
            })
            .collect::<Vec<_>>();
        let c = dealer(&d.path().join("shares"), &paths).unwrap();
        let a = derive(&c, "regtest", 2).unwrap();
        let b = derive(&c, "regtest", 2).unwrap();
        assert_ne!(a.nk, b.nk);
        assert_ne!(a.rivk, b.rivk);
        assert_ne!(a.uivk, b.uivk);
        assert_eq!(a.addresses.len(), 2);
        assert!(a.ufvk.starts_with("uviewregtest"));
        let net = LocalNetwork {
            overwinter: None,
            sapling: None,
            blossom: None,
            heartwood: None,
            canopy: None,
            nu5: None,
            nu6: None,
            nu6_1: None,
            nu6_2: None,
            nu6_3: Some(BlockHeight::from_u32(1)),
            nu7: None,
        };
        let parsed = zcash_keys::keys::UnifiedIncomingViewingKey::decode(&net, &a.uivk).unwrap();
        for (j, address) in a.addresses.iter().enumerate() {
            let expected = UnifiedAddress::from_receivers(
                Some(parsed.orchard().as_ref().unwrap().address_at(j as u32)),
                None,
                None,
                None,
                None,
            )
            .unwrap()
            .encode(&net);
            assert_eq!(*address, expected);
        }
        let public = serde_json::to_string(&c).unwrap();
        assert!(!public.contains("signing_share"));
        assert_eq!(
            std::fs::read_dir(d.path().join("shares")).unwrap().count(),
            4
        );
    }
}

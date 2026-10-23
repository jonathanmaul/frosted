//! Every spend uses fresh FROST nonces; shares never leave this process.
use crate::{
    keys::Committee,
    secrets::{Secrets, read_private},
};
use anyhow::{Context, Result, ensure};
use rand_core::OsRng;
use reddsa::frost::redpallas as frost;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SigningRequest {
    pub pool: String,
    pub action_index: usize,
    pub sighash_hex: String,
    pub alpha_hex: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpendSignature {
    pub pool: String,
    pub action_index: usize,
    pub signature_hex: String,
}
pub fn load_member(
    share_path: &Path,
    key_path: &Path,
    committee: &Committee,
) -> Result<frost::keys::KeyPackage> {
    let name = share_path
        .file_name()
        .and_then(|n| n.to_str())
        .context("invalid share filename")?;
    let id = name
        .strip_suffix(".share.enc")
        .context("invalid share filename")?;
    let identifier = frost::Identifier::deserialize(&hex::decode(id)?)?;
    let plaintext = Secrets::load(key_path)?.open(
        &format!("frost/{}/{id}", committee.group_id),
        &read_private(share_path)?,
    )?;
    let package: frost::keys::KeyPackage = serde_json::from_slice(&plaintext)?;
    ensure!(
        *package.identifier() == identifier,
        "share identifier mismatch"
    );
    validate_member(committee, &package)?;
    Ok(package)
}
fn validate_member(c: &Committee, k: &frost::keys::KeyPackage) -> Result<()> {
    ensure!(
        k.verifying_key() == c.public_key_package.verifying_key() && *k.min_signers() == 2,
        "wrong committee"
    );
    ensure!(
        c.public_key_package.verifying_shares().get(k.identifier()) == Some(k.verifying_share()),
        "wrong member"
    );
    Ok(())
}
#[allow(deprecated)] // Orchard's alpha is fixed by the PCZT; seed-derived randomizers cannot replace it.
pub fn sign(
    committee: &Committee,
    members: &[frost::keys::KeyPackage],
    requests: &[SigningRequest],
) -> Result<Vec<SpendSignature>> {
    ensure!(members.len() == 2, "exactly two members required");
    ensure!(!requests.is_empty(), "no spend requests");
    ensure!(
        committee.public_key_package.verifying_shares().len() == 3
            && committee.public_key_package.min_signers() == Some(2),
        "expected 2-of-3 committee"
    );
    ensure!(
        members[0].identifier() != members[1].identifier(),
        "duplicate member"
    );
    for member in members {
        validate_member(committee, member)?;
    }
    let mut seen = BTreeSet::new();
    let mut result = vec![];
    for request in requests {
        ensure!(
            matches!(request.pool.as_str(), "orchard" | "ironwood"),
            "invalid pool"
        );
        ensure!(
            seen.insert((&request.pool, request.action_index)),
            "duplicate action"
        );
        let message: [u8; 32] = hex::decode(&request.sighash_hex)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("invalid sighash"))?;
        let randomizer =
            frost::rerandomized::Randomizer::deserialize(&hex::decode(&request.alpha_hex)?)?;
        let params = frost::rerandomized::RandomizedParams::from_randomizer(
            committee.public_key_package.verifying_key(),
            randomizer,
        );
        let mut nonces = vec![];
        let mut commitments = BTreeMap::new();
        for member in members {
            let (nonce, commitment) = frost::round1::commit(member.signing_share(), &mut OsRng);
            nonces.push(nonce);
            commitments.insert(*member.identifier(), commitment);
        }
        let package = frost::SigningPackage::new(commitments, &message);
        let shares = members
            .iter()
            .zip(&nonces)
            .map(|(member, nonce)| {
                Ok((
                    *member.identifier(),
                    frost_rerandomized::sign(&package, nonce, member, randomizer)?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        let signature = frost::rerandomized::aggregate(
            &package,
            &shares,
            &committee.public_key_package,
            &params,
        )?;
        params
            .randomized_verifying_key()
            .verify(&message, &signature)?;
        result.push(SpendSignature {
            pool: request.pool.clone(),
            action_index: request.action_index,
            signature_hex: hex::encode(signature.serialize()?),
        });
    }
    Ok(result)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{keys, secrets::write_private_new};
    #[test]
    fn encrypted_two_of_three_signs_randomized_message_and_rejects_wrong_members() {
        let d = tempfile::tempdir().unwrap();
        let keys = (1..=3)
            .map(|i| {
                let p = d.path().join(format!("key{i}"));
                write_private_new(&p, &[i; 32]).unwrap();
                p
            })
            .collect::<Vec<_>>();
        let dir = d.path().join("shares");
        let c = keys::dealer(&dir, &keys).unwrap();
        let mut paths = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.to_string_lossy().ends_with(".share.enc"))
            .collect::<Vec<_>>();
        paths.sort();
        let a = load_member(&paths[0], &keys[0], &c).unwrap();
        let b = load_member(&paths[1], &keys[1], &c).unwrap();
        assert!(load_member(&paths[0], &keys[1], &c).is_err());
        let request = SigningRequest {
            pool: "ironwood".into(),
            action_index: 0,
            sighash_hex: hex::encode([42; 32]),
            alpha_hex: hex::encode({
                let mut x = [0; 32];
                x[0] = 5;
                x
            }),
        };
        let signatures = sign(&c, &[a.clone(), b], std::slice::from_ref(&request)).unwrap();
        assert_eq!(hex::decode(&signatures[0].signature_hex).unwrap().len(), 64);
        assert!(sign(&c, &[a.clone(), a], &[request]).is_err());
    }
}

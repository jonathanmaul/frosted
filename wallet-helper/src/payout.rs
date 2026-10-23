//! Exact-input, exact-output PCZT boundary. No wallet coin selection is permitted here.
use super::*;
use ff::PrimeField;
use nonempty::NonEmpty;
use pczt::{
    Pczt,
    roles::{low_level_signer, prover::Prover, signer::Signer},
};
use std::collections::{BTreeMap, BTreeSet};
use zcash_client_backend::{
    data_api::{InputSource, wallet::input_selection::LockFilter},
    fees::TransactionBalance,
    proposal::{Proposal, ShieldedInputs},
};
use zcash_primitives::transaction::TxId;
use zcash_protocol::PoolType;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub txid: String,
    pub action_index: u32,
    pub amount_zat: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Output {
    pub receiver_hex: String,
    pub amount_zat: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpendSignature {
    pub pool: String,
    pub action_index: usize,
    pub signature_hex: String,
}
#[derive(Serialize)]
pub struct SigningRequest {
    pub pool: String,
    pub action_index: usize,
    pub sighash_hex: String,
    pub alpha_hex: String,
}
#[derive(Serialize)]
pub struct OutputLocation {
    pub pool: String,
    pub action_index: usize,
    pub receiver_hex: String,
    pub amount_zat: u64,
}
#[derive(Default, Serialize)]
pub struct ResultFields {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pczt_b64: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub payout_outputs: Vec<OutputLocation>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub signing_requests: Vec<SigningRequest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry_height: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actual_actions: Option<usize>,
}
fn validate(r: &Request) -> Result<()> {
    ensure!(!r.inputs.is_empty(), "empty batch");
    let actions = r.inputs.len().max(r.outputs.len()).max(2);
    ensure!(
        actions <= r.max_actions.ok_or(anyhow!("missing action limit"))? as usize,
        "action limit exceeded"
    );
    let fee = r.fee_zat.ok_or(anyhow!("missing fee"))?;
    ensure!(fee >= 5000 * actions as u64, "fee mismatch");
    let mut seen = BTreeSet::new();
    let ins = r.inputs.iter().try_fold(0u64, |sum, i| {
        ensure!(seen.insert((&i.txid, i.action_index)), "duplicate input");
        ensure!(i.amount_zat > 0, "zero input");
        sum.checked_add(i.amount_zat).ok_or(anyhow!("overflow"))
    })?;
    let outs = r.outputs.iter().try_fold(0u64, |sum, o| {
        ensure!(o.amount_zat > 0, "zero output");
        sum.checked_add(o.amount_zat).ok_or(anyhow!("overflow"))
    })?;
    ensure!(outs.checked_add(fee) == Some(ins), "value mismatch");
    Ok(())
}
pub fn build(
    r: &Request,
    p: &Network,
    db: &mut WalletDb<&mut Connection, Network, SystemClock, UnwrapErr<SysRng>>,
    id: zcash_client_sqlite::AccountUuid,
) -> Result<ResultFields> {
    validate(r)?;
    let (target, anchor) = db
        .get_target_and_anchor_heights(NonZeroU32::new(1).unwrap())?
        .ok_or(anyhow!("missing anchor"))?;
    let ironwood = p
        .activation_height(NetworkUpgrade::Nu6_3)
        .is_some_and(|h| h <= target.into());
    // The indexer identifies Ironwood actions only; a same-index Orchard note is unrelated.
    ensure!(ironwood, "Ironwood is not active");
    let pool = ShieldedPool::Ironwood;
    let mut notes = vec![];
    for input in &r.inputs {
        let txid = TxId::from_hex(&input.txid).ok_or(anyhow!("invalid txid"))?;
        let note = db
            .get_spendable_note(
                &txid,
                pool,
                input.action_index,
                target,
                LockFilter::Policy(&Default::default()),
            )?
            .ok_or(anyhow!("planned note unavailable"))?;
        ensure!(
            u64::from(note.note().value()) == input.amount_zat,
            "planned input value mismatch"
        );
        notes.push(note);
    }
    let mut payments = vec![];
    for output in &r.outputs {
        let raw: [u8; 43] = hex::decode(&output.receiver_hex)?
            .try_into()
            .map_err(|_| anyhow!("invalid receiver"))?;
        let receiver = orchard::Address::from_raw_address_bytes(&raw)
            .into_option()
            .ok_or(anyhow!("invalid receiver"))?;
        let address = zcash_keys::address::UnifiedAddress::from_receivers(
            Some(receiver),
            None,
            None,
            None,
            None,
        )
        .ok_or(anyhow!("invalid address"))?;
        payments.push(
            zip321::Payment::new(
                zcash_address::ZcashAddress::from_str(&address.encode(p))?,
                Some(Zatoshis::from_u64(output.amount_zat)?),
                None,
                None,
                None,
                vec![],
            )
            .map_err(|_| anyhow!("invalid payment"))?,
        );
    }
    let pools = (0..payments.len())
        .map(|i| {
            (
                i,
                if ironwood {
                    PoolType::IRONWOOD
                } else {
                    PoolType::ORCHARD
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let proposal = Proposal::single_step(
        zip321::TransactionRequest::new(payments)?,
        pools,
        vec![],
        Some(ShieldedInputs::from_parts(
            NonEmpty::from_vec(notes).unwrap(),
        )),
        anchor,
        TransactionBalance::new(vec![], Zatoshis::from_u64(r.fee_zat.unwrap())?)?,
        StandardFeeRule::Zip317,
        target,
        ConfirmationsPolicy::MIN,
        false,
        ironwood,
    )?;
    let pczt = zcash_client_backend::data_api::wallet::create_pczt_from_proposal::<
        _,
        _,
        std::convert::Infallible,
        _,
        std::convert::Infallible,
        _,
    >(
        db,
        p,
        &SystemClock,
        &mut UnwrapErr(SysRng),
        id,
        OvkPolicy::Sender,
        &proposal,
        None,
        zcash_primitives::transaction::builder::BundlePadding::DEFAULT,
    )
    .map_err(|e| anyhow!("create PCZT: {e:?}"))?;
    let actions = pczt.orchard().actions().len() + pczt.ironwood().actions().len();
    ensure!(
        actions <= r.max_actions.unwrap() as usize,
        "actual action limit exceeded"
    );
    ensure!(
        5000 * actions.max(2) as u64 <= r.fee_zat.unwrap(),
        "actual fee mismatch"
    );
    let branch =
        zcash_protocol::consensus::BranchId::try_from(*pczt.global().consensus_branch_id())
            .map_err(|_| anyhow!("invalid branch"))?;
    let mut prover = Prover::new(pczt);
    if prover.requires_orchard_proof() || prover.requires_ironwood_proof() {
        let version =
            zcash_primitives::transaction::components::orchard::bundle_version_for_branch(
                branch,
                orchard::ValuePool::Orchard,
            )
            .ok_or(anyhow!("unsupported branch"))?;
        let pk = orchard::circuit::ProvingKey::build(version.circuit_version());
        if prover.requires_orchard_proof() {
            prover = prover
                .create_orchard_proof(UnwrapErr(SysRng), &pk)
                .map_err(|e| anyhow!("Orchard proof: {e:?}"))?;
        }
        if prover.requires_ironwood_proof() {
            prover = prover
                .create_ironwood_proof(UnwrapErr(SysRng), &pk)
                .map_err(|e| anyhow!("Ironwood proof: {e:?}"))?;
        }
    }
    let pczt = prover.finish();
    let payout_outputs = [("orchard", pczt.orchard()), ("ironwood", pczt.ironwood())]
        .into_iter()
        .flat_map(|(pool, bundle)| {
            bundle
                .actions()
                .iter()
                .enumerate()
                .filter_map(
                    move |(i, a)| match (a.output().recipient(), a.output().value()) {
                        (Some(receiver), Some(value)) if *value > 0 => Some(OutputLocation {
                            pool: pool.into(),
                            action_index: i,
                            receiver_hex: hex::encode(receiver),
                            amount_zat: *value,
                        }),
                        _ => None,
                    },
                )
        })
        .collect::<Vec<_>>();
    let mut actual = payout_outputs
        .iter()
        .map(|o| (&o.receiver_hex, o.amount_zat))
        .collect::<Vec<_>>();
    actual.sort();
    let mut expected = r
        .outputs
        .iter()
        .map(|o| (&o.receiver_hex, o.amount_zat))
        .collect::<Vec<_>>();
    expected.sort();
    ensure!(actual == expected, "PCZT outputs differ from plan");
    let sighash = Signer::new(pczt.clone())
        .map_err(|e| anyhow!("signer: {e:?}"))?
        .shielded_sighash();
    let mut requests = vec![];
    let pczt = low_level_signer::Signer::new(pczt)
        .sign_ironwood_with(
            |_, bundle, _| -> Result<(), low_level_signer::OrchardParseError> {
                for (i, a) in bundle.actions().iter().enumerate() {
                    if a.spend().spend_auth_sig().is_none()
                        && let Some(alpha) = a.spend().alpha()
                    {
                        requests.push(SigningRequest {
                            pool: "ironwood".into(),
                            action_index: i,
                            sighash_hex: hex::encode(sighash),
                            alpha_hex: hex::encode(alpha.to_repr()),
                        });
                    }
                }
                Ok(())
            },
        )
        .map_err(|e| anyhow!("sign metadata: {e:?}"))?
        .sign_orchard_with(
            |_, bundle, _| -> Result<(), low_level_signer::OrchardParseError> {
                for (i, a) in bundle.actions().iter().enumerate() {
                    if a.spend().spend_auth_sig().is_none()
                        && let Some(alpha) = a.spend().alpha()
                    {
                        requests.push(SigningRequest {
                            pool: "orchard".into(),
                            action_index: i,
                            sighash_hex: hex::encode(sighash),
                            alpha_hex: hex::encode(alpha.to_repr()),
                        });
                    }
                }
                Ok(())
            },
        )
        .map_err(|e| anyhow!("sign metadata: {e:?}"))?
        .finish();
    ensure!(requests.len() == r.inputs.len(), "unexpected real spends");
    Ok(ResultFields {
        payout_outputs,
        expiry_height: Some(*pczt.global().expiry_height()),
        actual_actions: Some(actions),
        pczt_b64: Some(STANDARD.encode(pczt.serialize().map_err(|e| anyhow!("serialize: {e:?}"))?)),
        signing_requests: requests,
    })
}
pub fn extract(
    r: &Request,
    db: &mut WalletDb<&mut Connection, Network, SystemClock, UnwrapErr<SysRng>>,
) -> Result<(String, String)> {
    let bytes = STANDARD.decode(r.pczt_b64.as_ref().ok_or(anyhow!("missing PCZT"))?)?;
    let pczt = Pczt::parse(&bytes).map_err(|e| anyhow!("PCZT: {e:?}"))?;
    let mut signer = Signer::new(pczt).map_err(|e| anyhow!("signer: {e:?}"))?;
    let mut seen = BTreeSet::new();
    for sig in &r.signatures {
        ensure!(
            seen.insert((&sig.pool, sig.action_index)),
            "duplicate signature"
        );
        let raw: [u8; 64] = hex::decode(&sig.signature_hex)?
            .try_into()
            .map_err(|_| anyhow!("invalid signature"))?;
        let sigbytes = raw.into();
        match sig.pool.as_str() {
            "ironwood" => signer.apply_ironwood_signature(sig.action_index, sigbytes),
            "orchard" => signer.apply_orchard_signature(sig.action_index, sigbytes),
            _ => return Err(anyhow!("invalid pool")),
        }
        .map_err(|e| anyhow!("apply signature: {e:?}"))?;
    }
    let prover = zcash_proofs::prover::LocalTxProver::bundled();
    let (spend, output) = prover.verifying_keys();
    let txid =
        zcash_client_backend::data_api::wallet::extract_and_store_transaction_from_pczt::<_, ()>(
            db,
            &SystemClock,
            &mut UnwrapErr(SysRng),
            signer.finish(),
            Some((&spend, &output)),
            None,
        )
        .map_err(|e| anyhow!("extract: {e:?}"))?;
    let tx = db
        .get_transaction(txid)?
        .ok_or(anyhow!("missing extracted transaction"))?;
    let mut bytes = vec![];
    tx.write(&mut bytes)?;
    Ok((hex::encode(bytes), txid.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> Request {
        serde_json::from_value(serde_json::json!({"op":"payout_build","network":"regtest","rpc_url":"http://localhost:18232","zaino_url":"http://localhost:18132","birthday_height":1,"inputs":[{"txid":"00".repeat(32),"action_index":0,"amount_zat":100000}],"outputs":[{"receiver_hex":"00".repeat(43),"amount_zat":90000}],"fee_zat":10000,"max_actions":40})).unwrap()
    }
    #[test]
    fn upstream_proposal_accepts_fee_only_ironwood_spend() {
        use orchard::{
            keys::{FullViewingKey, Scope, SpendingKey},
            note::{NoteVersion, RandomSeed, Rho},
            value::NoteValue,
        };
        use zcash_client_backend::wallet::{Note, ReceivedNote};
        let sk = SpendingKey::from_bytes([7; 32]).unwrap();
        let address = FullViewingKey::from(&sk).address_at(0u32, Scope::External);
        let rho = Rho::from_bytes(&[0; 32]).unwrap();
        let seed = RandomSeed::from_bytes([1; 32], &rho).unwrap();
        let note = orchard::Note::from_parts(
            address,
            NoteValue::from_raw(10_000),
            rho,
            seed,
            NoteVersion::V3,
        )
        .unwrap();
        let received = ReceivedNote::from_parts(
            (),
            TxId::from_bytes([0; 32]),
            0,
            Note::Orchard {
                note,
                pool: orchard::ValuePool::Ironwood,
            },
            zip32::Scope::External,
            0u64.into(),
            Some(BlockHeight::from_u32(1)),
            None,
        );
        let request = zip321::TransactionRequest::new(vec![]).unwrap();
        let proposal = Proposal::single_step(
            request,
            BTreeMap::new(),
            vec![],
            Some(ShieldedInputs::from_parts(NonEmpty::singleton(received))),
            BlockHeight::from_u32(10),
            TransactionBalance::new(vec![], Zatoshis::from_u64(10_000).unwrap()).unwrap(),
            StandardFeeRule::Zip317,
            11u32.into(),
            ConfirmationsPolicy::MIN,
            false,
            true,
        )
        .unwrap();
        assert_eq!(proposal.steps().len(), 1);
    }
    #[test]
    fn rejects_value_mismatch_duplicate_notes_and_underpaid_actions() {
        let mut r = request();
        assert!(validate(&r).is_ok());
        r.outputs[0].amount_zat += 1;
        assert!(validate(&r).is_err());
        let mut r = request();
        r.inputs.push(Input {
            txid: r.inputs[0].txid.clone(),
            action_index: 0,
            amount_zat: 100000,
        });
        assert!(validate(&r).is_err());
        let mut r = request();
        r.max_actions = Some(1);
        assert!(validate(&r).is_err());
        let mut r = request();
        r.fee_zat = Some(5000);
        r.outputs[0].amount_zat = 95000;
        assert!(validate(&r).is_err());
    }
}

//! Test fixtures for the regtest integration test (scripts/it-indexer.sh). Test data only.
//!
//!   it_fixture market <uivk> <addr0> <addr1> <birthday> <close> <resolve> <timeout> <market_id>
//!   it_fixture frst1 <market_id>            FRST1 memo (dummy envelope) for that market
//!   it_fixture frstm1 <definition.json>     FRSTM1 announcement memo with its rules_hash
//!   it_fixture registry                     regtest UIVK and address of the test Persona Registry
//!   it_fixture escrow <seed_hex32>          regtest UIVK and outcome addresses 0..2 for a test escrow
//!   it_fixture frstp1 <k_root_hex>          FRSTP1 memo and persona_id
//!   it_fixture frst1-tagged <market_id> <k_root_hex> <outcome> <amount_zat> <epk_byte_hex>
//!                                           FRST1 memo (dummy envelope) carrying Thaw Tag v2 for that bet
use anyhow::{Context, Result, bail};
use orchard::keys::{FullViewingKey, SpendingKey};
use zcash_glass::hash::{
    MarketDefinition, OutcomeDefinition, PoolThresholdParams, ResolutionParams, ResolutionType,
    rules_hash,
};
use zcash_glass::memo::{Frst1Header, Frst1Payload, encode_frst1, encode_frstm1, encode_frstp1};
use zcash_glass::scan::{ActivationHeights, ScanNetwork};
use zcash_glass::thaw;
use zcash_keys::address::UnifiedAddress;
use zcash_keys::keys::UnifiedFullViewingKey;

fn main() -> Result<()> {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let arg = |i: usize| a.get(i).cloned().context("missing argument");
    let net = ScanNetwork::new("regtest", ActivationHeights::default())?;
    match a.first().map(String::as_str) {
        Some("market") => {
            let n = |i: usize| -> Result<u32> { Ok(arg(i)?.parse()?) };
            let def = MarketDefinition {
                version: 1,
                params_version: 1,
                network: "regtest".into(),
                market_id: arg(8)?,
                question: "Will the Ironwood pool hold >= 1 ZEC at the resolve height?".into(),
                outcomes: vec![
                    OutcomeDefinition {
                        outcome_index: 0,
                        label: "YES".into(),
                        address: arg(2)?,
                    },
                    OutcomeDefinition {
                        outcome_index: 1,
                        label: "NO".into(),
                        address: arg(3)?,
                    },
                ],
                resolution_type: ResolutionType::PoolThreshold,
                resolution_params: ResolutionParams::PoolThreshold(PoolThresholdParams {
                    pool: "ironwood".into(),
                    comparator: "gte".into(),
                    threshold_zat: 100_000_000,
                }),
                birthday_height: n(4)?,
                close_height: n(5)?,
                resolve_height: n(6)?,
                fee_bps: 100,
                min_bet_zat: 1_000_000,
                refund_fee_zat: 5_000,
                min_confirmations: 3,
                sweep_threshold_zat: 5_000,
                announce_timeout_blocks: n(7)?,
                uivk: arg(1)?,
                envelope_pk: "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".into(),
                treasury_address: "utreasury-fixture".into(),
            };
            println!("{}", serde_json::to_string(&def)?);
        }
        Some("frst1") => {
            let mut id = [0u8; 16];
            hex::decode_to_slice(arg(1)?, &mut id)?;
            let payload = Frst1Payload {
                header: Frst1Header::new(id, None),
                env_epk: [7; 32],
                env_nonce: [8; 24],
                env_ciphertext: [9; 67],
            };
            println!("{}", encode_frst1(&payload)?);
        }
        Some("frst1-tagged") => {
            let mut id = [0u8; 16];
            hex::decode_to_slice(arg(1)?, &mut id)?;
            let mut k = [0u8; 32];
            hex::decode_to_slice(arg(2)?, &mut k)?;
            let (outcome, amount): (u8, u64) = (arg(3)?.parse()?, arg(4)?.parse()?);
            let mut byte = [0u8; 1];
            hex::decode_to_slice(arg(5)?, &mut byte)?;
            let env_epk = [byte[0]; 32];
            let tag = thaw::tag(&k, &id, outcome, amount, &env_epk);
            let payload = Frst1Payload {
                header: Frst1Header::new(id, Some(tag)),
                env_epk,
                env_nonce: [8; 24],
                env_ciphertext: [9; 67],
            };
            println!("{}", encode_frst1(&payload)?);
        }
        Some("frstm1") => {
            let def: MarketDefinition = serde_json::from_str(&std::fs::read_to_string(arg(1)?)?)?;
            println!("{}", encode_frstm1(&rules_hash(&def)?));
        }
        Some("registry") => {
            let fvk = FullViewingKey::from(&SpendingKey::from_bytes([0x33; 32]).unwrap());
            let addr = fvk.address_at(0u32, orchard::keys::Scope::External);
            let ufvk = UnifiedFullViewingKey::from_orchard_fvk(fvk)
                .map_err(|e| anyhow::anyhow!("{e:?}"))?;
            let ua =
                UnifiedAddress::from_receivers(Some(addr), None, None, None, None).context("UA")?;
            println!(
                "{} {}",
                ufvk.to_unified_incoming_viewing_key().encode(&net).unwrap(),
                ua.encode(&net)
            );
        }
        Some("escrow") => {
            let mut seed = [0u8; 32];
            hex::decode_to_slice(arg(1)?, &mut seed)?;
            let sk = SpendingKey::from_bytes(seed)
                .into_option()
                .context("seed is not a valid key")?;
            let fvk = FullViewingKey::from(&sk);
            let mut out = vec![];
            for j in 0u32..3 {
                let addr = fvk.address_at(j, orchard::keys::Scope::External);
                out.push(
                    UnifiedAddress::from_receivers(Some(addr), None, None, None, None)
                        .context("UA")?
                        .encode(&net),
                );
            }
            let ufvk = UnifiedFullViewingKey::from_orchard_fvk(fvk)
                .map_err(|e| anyhow::anyhow!("{e:?}"))?;
            println!(
                "{} {}",
                ufvk.to_unified_incoming_viewing_key().encode(&net).unwrap(),
                out.join(" ")
            );
        }
        Some("frstp1") => {
            let mut k = [0u8; 32];
            hex::decode_to_slice(arg(1)?, &mut k)?;
            let c = thaw::commitment(&k);
            println!("{} {}", encode_frstp1(&c), thaw::persona_id(&c));
        }
        _ => bail!("see module docs"),
    }
    Ok(())
}

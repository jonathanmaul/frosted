//! Chain scanning with a Market UIVK (feature `zcash`; docs/SPIKES.md §M0.2, Appendix M4).
//!
//! Uses the stable librustzcash group (ADR-0015): parse a v6 transaction with the branch id for
//! its height, then trial-decrypt every Ironwood action with
//! `try_note_decryption(&IronwoodDomain::for_action(a), &ivk.prepare(), a)`.

use orchard::keys::{IncomingViewingKey, PreparedIncomingViewingKey};
use orchard::note_encryption::IronwoodDomain;
use thiserror::Error;
use zcash_keys::keys::UnifiedIncomingViewingKey;
use zcash_note_encryption::try_note_decryption;
use zcash_primitives::transaction::Transaction;
use zcash_protocol::consensus::{
    BlockHeight, BranchId, MainNetwork, NetworkType, NetworkUpgrade, Parameters, TestNetwork,
};
use zcash_protocol::local_consensus::LocalNetwork;

#[derive(Debug, Error)]
pub enum ScanError {
    #[error("unknown network {0:?}")]
    UnknownNetwork(String),
    #[error("invalid UIVK: {0}")]
    InvalidUivk(String),
    #[error("UIVK has no Orchard item")]
    NoOrchardItem,
    #[error("transaction parse error: {0}")]
    Parse(#[from] std::io::Error),
}

/// Activation heights reported by the validator's `getblockchaininfo` (ADR-0016).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ActivationHeights {
    pub overwinter: Option<u32>,
    pub sapling: Option<u32>,
    pub blossom: Option<u32>,
    pub heartwood: Option<u32>,
    pub canopy: Option<u32>,
    pub nu5: Option<u32>,
    pub nu6: Option<u32>,
    pub nu6_1: Option<u32>,
    pub nu6_2: Option<u32>,
    pub nu6_3: Option<u32>,
}

impl ActivationHeights {
    /// Sets the height for an upgrade name as reported in `getblockchaininfo.upgrades[].name`.
    /// Unknown names (for example `NU7`, which librustzcash does not model here) are ignored.
    pub fn set(&mut self, name: &str, height: u32) {
        let slot = match name {
            "Overwinter" => &mut self.overwinter,
            "Sapling" => &mut self.sapling,
            "Blossom" => &mut self.blossom,
            "Heartwood" => &mut self.heartwood,
            "Canopy" => &mut self.canopy,
            "NU5" => &mut self.nu5,
            "NU6" => &mut self.nu6,
            "NU6.1" => &mut self.nu6_1,
            "NU6.2" => &mut self.nu6_2,
            "NU6.3" => &mut self.nu6_3,
            _ => return,
        };
        *slot = Some(height);
    }
}

/// Consensus parameters for scanning. Mainnet and testnet use the library constants; regtest is
/// built from the validator's activation heights.
#[derive(Clone, Copy, Debug)]
pub enum ScanNetwork {
    Main,
    Test,
    Regtest(LocalNetwork),
}

impl ScanNetwork {
    /// `name` is `market_definition.network` (`mainnet`, `testnet`, `regtest`).
    pub fn new(name: &str, heights: ActivationHeights) -> Result<Self, ScanError> {
        let h = |v: Option<u32>| v.map(BlockHeight::from_u32);
        match name {
            "mainnet" | "main" => Ok(ScanNetwork::Main),
            "testnet" | "test" => Ok(ScanNetwork::Test),
            "regtest" => Ok(ScanNetwork::Regtest(LocalNetwork {
                overwinter: h(heights.overwinter),
                sapling: h(heights.sapling),
                blossom: h(heights.blossom),
                heartwood: h(heights.heartwood),
                canopy: h(heights.canopy),
                nu5: h(heights.nu5),
                nu6: h(heights.nu6),
                nu6_1: h(heights.nu6_1),
                nu6_2: h(heights.nu6_2),
                nu6_3: h(heights.nu6_3),
            })),
            other => Err(ScanError::UnknownNetwork(other.to_string())),
        }
    }

    /// `BranchId::for_height` for this network.
    pub fn branch_id(&self, height: u32) -> BranchId {
        BranchId::for_height(self, BlockHeight::from_u32(height))
    }
}

impl Parameters for ScanNetwork {
    fn network_type(&self) -> NetworkType {
        match self {
            ScanNetwork::Main => MainNetwork.network_type(),
            ScanNetwork::Test => TestNetwork.network_type(),
            ScanNetwork::Regtest(n) => n.network_type(),
        }
    }

    fn activation_height(&self, nu: NetworkUpgrade) -> Option<BlockHeight> {
        match self {
            ScanNetwork::Main => MainNetwork.activation_height(nu),
            ScanNetwork::Test => TestNetwork.activation_height(nu),
            ScanNetwork::Regtest(n) => n.activation_height(nu),
        }
    }
}

/// The Orchard IVK of a Market (or of the Persona Registry), decoded from its published UIVK.
#[derive(Clone, Debug)]
pub struct MarketViewingKey {
    ivk: IncomingViewingKey,
    prepared: PreparedIncomingViewingKey,
}

impl MarketViewingKey {
    pub fn from_uivk(network: &ScanNetwork, uivk: &str) -> Result<Self, ScanError> {
        let uivk =
            UnifiedIncomingViewingKey::decode(network, uivk).map_err(ScanError::InvalidUivk)?;
        let ivk = uivk.orchard().clone().ok_or(ScanError::NoOrchardItem)?;
        let prepared = ivk.prepare();
        Ok(MarketViewingKey { ivk, prepared })
    }

    /// Builds the key from an Orchard IVK directly (coordinator side, tests).
    pub fn from_orchard_ivk(ivk: IncomingViewingKey) -> Self {
        let prepared = ivk.prepare();
        MarketViewingKey { ivk, prepared }
    }

    /// Raw 43-byte Orchard receiver at diversifier index `j` (Outcome Address `j`).
    pub fn receiver_at(&self, j: u32) -> [u8; 43] {
        self.ivk.address_at(j).to_raw_address_bytes()
    }
}

/// A parsed transaction with what the scanner needs.
pub struct ParsedTx {
    tx: Transaction,
}

impl ParsedTx {
    /// Parses a raw transaction mined at `height` (`getrawtransaction(txid, 0)` bytes).
    pub fn parse(bytes: &[u8], height: u32, network: &ScanNetwork) -> Result<Self, ScanError> {
        let tx = Transaction::read(bytes, network.branch_id(height))?;
        Ok(ParsedTx { tx })
    }

    /// Txid in internal byte order (as used by `zcash-glass::hash::note_ref`).
    pub fn txid(&self) -> [u8; 32] {
        *self.tx.txid().as_ref()
    }

    /// Z-only Guard: true if the transaction spends any transparent input.
    pub fn has_transparent_input(&self) -> bool {
        self.tx
            .transparent_bundle()
            .is_some_and(|b| !b.vin.is_empty())
    }

    /// Trial-decrypts every Ironwood action with `key`.
    pub fn decrypt(&self, key: &MarketViewingKey) -> Vec<DecryptedNote> {
        let Some(bundle) = self.tx.ironwood_bundle() else {
            return Vec::new();
        };
        bundle
            .actions()
            .iter()
            .enumerate()
            .filter_map(|(i, action)| {
                let domain = IronwoodDomain::for_action(action);
                let (note, recipient, memo) = try_note_decryption(&domain, &key.prepared, action)?;
                let diversifier_index = key.ivk.diversifier_index(&recipient).and_then(|d| {
                    let b = d.as_bytes();
                    // Diversifier indices are 11 bytes; Frosted only uses small indices.
                    b[8..]
                        .iter()
                        .all(|x| *x == 0)
                        .then(|| u64::from_le_bytes(b[..8].try_into().expect("8 bytes")))
                });
                Some(DecryptedNote {
                    action_index: i as u32,
                    value_zat: note.value().inner(),
                    diversifier_index,
                    memo,
                })
            })
            .collect()
    }
}

/// One decrypted Ironwood output of a Market escrow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecryptedNote {
    pub action_index: u32,
    pub value_zat: u64,
    pub diversifier_index: Option<u64>,
    pub memo: [u8; 512],
}

impl DecryptedNote {
    /// ZIP 302 text memo (first byte ≤ 0xF4), trailing zero bytes removed; `None` otherwise.
    pub fn memo_text(&self) -> Option<&str> {
        if self.memo[0] > 0xF4 {
            return None;
        }
        let end = self.memo.iter().rposition(|b| *b != 0).map_or(0, |p| p + 1);
        std::str::from_utf8(&self.memo[..end]).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn heights() -> ActivationHeights {
        let mut h = ActivationHeights::default();
        for n in [
            "Overwinter",
            "Sapling",
            "Blossom",
            "Heartwood",
            "Canopy",
            "NU5",
            "NU6",
            "NU6.1",
            "NU6.2",
        ] {
            h.set(n, 1);
        }
        h.set("NU6.3", 10);
        h.set("NU7", 2); // ignored: not modeled
        h
    }

    #[test]
    fn branch_id_follows_validator_heights() {
        let net = ScanNetwork::new("regtest", heights()).unwrap();
        assert_eq!(net.branch_id(5), BranchId::Nu6_2);
        assert_eq!(net.branch_id(10), BranchId::Nu6_3);
        assert_eq!(net.branch_id(1000), BranchId::Nu6_3);
    }

    #[test]
    fn unknown_network_rejected() {
        assert!(ScanNetwork::new("signet", heights()).is_err());
    }

    #[test]
    fn memo_text_rules() {
        let mut memo = [0u8; 512];
        memo[..6].copy_from_slice(b"FRST1:");
        let n = DecryptedNote {
            action_index: 0,
            value_zat: 1,
            diversifier_index: Some(0),
            memo,
        };
        assert_eq!(n.memo_text(), Some("FRST1:"));
        let mut binary = n.clone();
        binary.memo[0] = 0xF6; // "no memo" marker
        assert_eq!(binary.memo_text(), None);
    }
}

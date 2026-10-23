//! The two things the verifier reads. `Node` is the verifier's own chain; `Api` is the public
//! Frosted data being checked. Production code uses HTTP/RPC; tests and the server-side check
//! use other implementations of the same traits.

use std::future::Future;

use anyhow::Result;
use serde_json::Value;
use zcash_glass::{hash::MarketDefinition, memo::Frst1Payload, status::BetStatus};

/// One decrypted escrow note, classified at the height it was mined (before confirmations).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    pub note_ref: String,
    pub outcome_index: Option<u32>,
    pub amount_zat: u64,
    pub block_height: u32,
    pub tx_index: u32,
    pub action_index: u32,
    /// `Pending` until enough confirmations; see [`Note::status_at`].
    pub status: BetStatus,
    pub is_announcement: bool,
    pub payload: Option<Frst1Payload>,
    pub bet_fingerprint: Option<String>,
}

impl Note {
    /// Status when the chain tip is `tip`: `Pending` becomes `Counted` after `min_confirmations`
    /// (J-3), and drops back if the tip is lower.
    pub fn status_at(&self, tip: u32, min_confirmations: u32) -> BetStatus {
        let confirmed =
            zcash_glass::validate::confirmations(self.block_height, tip) >= min_confirmations;
        match self.status {
            BetStatus::Pending | BetStatus::Counted => {
                if confirmed {
                    BetStatus::Counted
                } else {
                    BetStatus::Pending
                }
            }
            other => other,
        }
    }
}

/// Everything one scan of a market's escrow finds on the verifier's node.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scan {
    pub tip: u32,
    pub notes: Vec<Note>,
    /// First valid FRSTM1 for this market's `rules_hash`: `(note_ref, height)`.
    pub announcement: Option<(String, u32)>,
    /// `rules_hash` carried by the first FRSTM1 memo sent to outcome 0, valid or not.
    pub first_frstm1_hash: Option<[u8; 32]>,
}

/// A Persona registration the registry key decrypted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Registration {
    pub commitment: [u8; 32],
    pub height: u32,
}

pub trait Node {
    /// Best guess from `getblockchaininfo.chain` (`test` cannot tell testnet from regtest).
    fn network_name(&self) -> impl Future<Output = Result<String>> + Send;
    fn tip(&self) -> impl Future<Output = Result<u32>> + Send;
    /// Scans `birthday_height..=upto` with the market UIVK.
    fn scan(
        &self,
        def: &MarketDefinition,
        rules_hash: &[u8; 32],
        upto: u32,
    ) -> impl Future<Output = Result<Scan>> + Send;
    /// `getblock(height, 1)` and `getblockchaininfo`, as the resolution rule reads them.
    fn resolution_data(&self, height: u32) -> impl Future<Output = Result<(Value, Value)>> + Send;
    /// FRSTP1 registrations seen by the Persona Registry UIVK in `birthday..=upto`.
    fn registrations(
        &self,
        uivk: &str,
        network: &str,
        birthday: u32,
        upto: u32,
    ) -> impl Future<Output = Result<Vec<Registration>>> + Send;
}

/// Public Frosted data. `Null` means "published nothing yet" for resolution and payout.
pub trait Api {
    fn market(&self, id: &str) -> impl Future<Output = Result<Value>> + Send;
    fn odds(&self, id: &str) -> impl Future<Output = Result<Value>> + Send;
    fn resolution(&self, id: &str) -> impl Future<Output = Result<Value>> + Send;
    /// Every non-superseded plan of the market (`GET /v1/markets/{id}/payouts`), oldest first.
    fn payouts(&self, id: &str) -> impl Future<Output = Result<Vec<Value>>> + Send;
    fn market_ids(&self) -> impl Future<Output = Result<Vec<String>>> + Send;
    fn indexer_height(&self) -> impl Future<Output = Result<u32>> + Send;
    /// Public bets of a market (`note_ref`s are enough to locate a bet).
    fn bets(&self, id: &str) -> impl Future<Output = Result<Value>> + Send;
    fn persona(&self, id: &str) -> impl Future<Output = Result<Option<Value>>> + Send;
    fn thaw_report(&self, id: &str) -> impl Future<Output = Result<Option<Value>>> + Send;
    fn registry(&self) -> impl Future<Output = Result<Option<Value>>> + Send;
}

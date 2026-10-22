use serde::{Deserialize, Serialize};
use zcash_glass::hash::{MarketDefinition, Plan};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Market {
    pub market_definition: MarketDefinition,
    pub rules_hash: String,
    pub status: String,
    pub announce_height: Option<u32>,
}

#[derive(Serialize)]
pub struct PublicBet {
    pub note_ref: String,
    pub bet_fingerprint: Option<String>,
    pub outcome_index: Option<u32>,
    pub amount_zat: u64,
    pub block_height: u32,
    pub status: String,
    pub tagged: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Vote {
    pub member_id: String,
    pub action: String,
    pub outcome_index: Option<u32>,
    pub evidence_url: Option<String>,
    pub created_at: u64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub source: String,
    pub height: u32,
    pub block_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pool: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chain_value_zat: Option<u64>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resolution {
    pub market_id: String,
    pub status: String,
    pub winning_outcome_index: Option<u32>,
    pub evidence: Option<Evidence>,
    pub votes: Vec<Vote>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PayoutSummary {
    pub winners: u32,
    pub refunds: u32,
    pub unclaimable: u32,
    pub swept: u32,
    pub uneconomic_dust: u32,
    pub operator_fee_zat: u64,
    pub tx_fee_zat: u64,
    pub distributed_zat: u64,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Signing {
    pub sessions_done: u32,
    pub sessions_total: u32,
    pub threshold: u32,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PayoutTx {
    pub txid: String,
    pub status: String,
    pub confirmed_height: Option<u32>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PayoutPublic {
    pub plan_hash: String,
    pub status: String,
    pub plan: Plan,
    pub summary: PayoutSummary,
    pub signing: Signing,
    pub txs: Vec<PayoutTx>,
}

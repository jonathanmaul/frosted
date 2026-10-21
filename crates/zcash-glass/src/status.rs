//! Bet statuses (docs/PROJECT_SPEC_FROSTED.md §23.2), shared by odds, payout and the indexer.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BetStatus {
    UneconomicDust,
    UnknownOutcome,
    RejectedTransparent,
    Late,
    IgnoredDust,
    InvalidMemo,
    Pending,
    Counted,
    Won,
    Lost,
    RefundQueued,
    Paid,
    Refunded,
    Unclaimable,
    Swept,
}

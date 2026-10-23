//! Public projection of scanner statuses onto resolution and payout plans, and the bet result a
//! Thaw Report shows. Shared by `frosted-api` and `frosted-verify` so both show the same thing.

use std::collections::BTreeMap;

use crate::hash::{ItemKind, Plan};
use crate::thaw::{BetResult, Settlement};

/// A published resolution reduced to what the projection needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResolutionView<'a> {
    pub status: &'a str,
    pub winning_outcome_index: Option<u32>,
}

/// `note_ref` -> (kind, amount, confirmed) over the non-failed plans of one market. A confirmed
/// plan wins over an unconfirmed one for the same note.
#[derive(Clone, Debug, Default)]
pub struct PlanItems(BTreeMap<String, (ItemKind, u64, bool)>);

impl PlanItems {
    /// `plans` are `(plan_status, plan)` pairs; `FAILED` plans are ignored.
    pub fn from_plans<'a>(plans: impl IntoIterator<Item = (&'a str, &'a Plan)>) -> Self {
        let mut items = BTreeMap::new();
        for (status, plan) in plans {
            if status == "FAILED" {
                continue;
            }
            let confirmed = status == "CONFIRMED";
            for item in &plan.items {
                let entry = items.entry(item.note_ref.clone()).or_insert((
                    item.kind,
                    item.amount_zat,
                    confirmed,
                ));
                if confirmed {
                    *entry = (item.kind, item.amount_zat, true);
                }
            }
        }
        PlanItems(items)
    }

    /// Amount of the Win item in a published plan, if any.
    pub fn win_amount(&self, note_ref: &str) -> Option<u64> {
        self.0
            .get(note_ref)
            .filter(|(kind, ..)| *kind == ItemKind::Win)
            .map(|(_, amount, _)| *amount)
    }
}

/// The status a visitor sees for a bet with scanner status `status`.
pub fn project_status(
    status: &str,
    outcome: Option<u32>,
    note_ref: &str,
    resolution: Option<ResolutionView<'_>>,
    items: &PlanItems,
) -> String {
    let mut status = status.to_owned();
    if status == "COUNTED"
        && let Some(r) = resolution
        && r.status == "RESOLVED"
        && let Some(winner) = r.winning_outcome_index
    {
        status = if outcome == Some(winner) {
            "WON"
        } else {
            "LOST"
        }
        .into();
    }
    if let Some((kind, _, confirmed)) = items.0.get(note_ref) {
        status = match (kind, confirmed) {
            (ItemKind::Win, true) => "PAID",
            (ItemKind::Win, false) => "WON",
            (ItemKind::Refund, true) => "REFUNDED",
            (ItemKind::Refund, false) => "REFUND_QUEUED",
            (ItemKind::Unclaimable, false) => "UNCLAIMABLE",
            (ItemKind::Sweep, false) => status.as_str(),
            (ItemKind::Unclaimable | ItemKind::Sweep, true) => "SWEPT",
        }
        .into();
    }
    status
}

/// Thaw Report result of a projected status.
pub fn result_of(projected: &str) -> BetResult {
    match projected {
        "WON" | "PAID" => BetResult::Won,
        "LOST" => BetResult::Lost,
        "REFUNDED" | "REFUND_QUEUED" => BetResult::Refunded,
        "COUNTED" | "PENDING" => BetResult::Pending,
        _ => BetResult::Other,
    }
}

/// Settlement of one bet for `thaw::report`.
pub fn settlement(
    status: &str,
    outcome: Option<u32>,
    note_ref: &str,
    resolution: Option<ResolutionView<'_>>,
    items: &PlanItems,
) -> Settlement {
    let result = result_of(&project_status(
        status, outcome, note_ref, resolution, items,
    ));
    Settlement {
        result,
        payout_zat: (result == BetResult::Won)
            .then(|| items.win_amount(note_ref))
            .flatten(),
    }
}

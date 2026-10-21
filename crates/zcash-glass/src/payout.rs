//! Payout Plan (docs/PROJECT_SPEC_FROSTED.md §19.6, ADR-0003, ADR-0004).
//!
//! Integer math in zatoshi. Winners are never charged transaction fees; all ZIP 317 fees are paid
//! from `operator_fee + swept_total + Σ refund_fee`, and planning fails closed with
//! `FEE_SHORTFALL` when that budget is not enough.

use thiserror::Error;

use crate::OrchardReceiver;
use crate::hash::{ItemKind, Plan, PlanItem, PlanKind, PlanResult, receiver_commitment};
use crate::params;
use crate::status::BetStatus;

/// ZIP 317 fee model (`[TO VERIFY]` A-7 / V-28; M0.4 may replace the defaults).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FeeModel {
    pub marginal_fee_zat: u64,
    pub grace_actions: u32,
    pub max_actions_per_tx: u32,
}

impl Default for FeeModel {
    fn default() -> Self {
        Self {
            marginal_fee_zat: params::MARGINAL_FEE_ZAT,
            grace_actions: params::GRACE_ACTIONS,
            max_actions_per_tx: params::MAX_ACTIONS_PER_TX,
        }
    }
}

/// What the coordinator learned when opening the note's Sealed Envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvelopeResult {
    Opened {
        receiver: OrchardReceiver,
        bet_salt: [u8; 8],
    },
    Unopenable,
    /// No FRST1 envelope (dust, invalid memo, unknown outcome, announcement).
    None,
}

/// One escrow note, already classified by the §19.5 rules.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteInput {
    pub note_ref: String,
    pub bet_fingerprint: Option<String>,
    pub outcome_index: Option<u32>,
    pub amount_zat: u64,
    pub block_height: u32,
    pub status: BetStatus,
    pub is_announcement: bool,
    pub envelope: EnvelopeResult,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanInput {
    pub market_id: String,
    pub plan_kind: PlanKind,
    pub plan_cutoff_height: u32,
    pub result: PlanResult,
    pub fee_bps: u32,
    pub refund_fee_zat: u64,
    pub fee_model: FeeModel,
    pub notes: Vec<NoteInput>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PayoutError {
    /// §22: `FEE_SHORTFALL`. No plan is produced.
    #[error("FEE_SHORTFALL: transaction fees {tx_fee_zat} exceed the fee budget {fee_budget_zat}")]
    FeeShortfall {
        tx_fee_zat: u64,
        fee_budget_zat: u64,
    },
    #[error("note {0} is not final (PENDING)")]
    NotFinal(String),
    #[error("note {0} has status {1:?}, which cannot appear in a payout plan")]
    UnexpectedStatus(String, BetStatus),
    #[error("note {0} is COUNTED but has no outcome_index")]
    MissingOutcome(String),
    #[error("duplicate note_ref {0}")]
    DuplicateNote(String),
    #[error("arithmetic overflow")]
    Overflow,
    #[error("invalid plan: {0}")]
    Invalid(String),
}

/// A shielded output the coordinator must create for a WIN or REFUND item. Never published.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrivateOutput {
    pub note_ref: String,
    pub kind: ItemKind,
    pub receiver: OrchardReceiver,
    pub amount_zat: u64,
}

/// Coordinator-only part of the plan (receivers in plaintext, kept in memory).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrivatePlanOutputs {
    pub outputs: Vec<PrivateOutput>,
    /// Single Treasury output per transaction batch: `fee_budget − tx_fee`.
    pub treasury_out_zat: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanSummary {
    pub winners: u32,
    pub refunds: u32,
    pub unclaimable: u32,
    pub swept: u32,
    pub uneconomic_dust: u32,
    pub excluded_after_cutoff: u32,
    pub pool_total_zat: u64,
    pub distributed_zat: u64,
    pub operator_fee_zat: u64,
    pub swept_total_zat: u64,
    pub fee_budget_zat: u64,
    pub tx_fee_zat: u64,
    pub tx_count: u32,
    pub spends: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanOutcome {
    /// Public plan (JCS input of `plan_hash`), items sorted by `note_ref`.
    pub public: Plan,
    pub private: PrivatePlanOutputs,
    pub summary: PlanSummary,
}

/// Fee estimate for spending `spends` notes into `payee_outputs` outputs, plus one Treasury/change
/// output per transaction when `with_treasury` is set, split so that every transaction stays
/// within `max_actions_per_tx`. Returns `(tx_fee_zat, tx_count)`.
pub fn estimate_fee(
    model: &FeeModel,
    spends: u32,
    payee_outputs: u32,
    with_treasury: bool,
) -> Result<(u64, u32), PayoutError> {
    if spends == 0 {
        return Ok((0, 0));
    }
    let max = model.max_actions_per_tx.max(2);
    let extra = u32::from(with_treasury);
    let mut n: u32 = 1;
    loop {
        let outputs = payee_outputs + n * extra;
        let need = spends.max(outputs).div_ceil(max);
        if need <= n {
            break;
        }
        n = need;
    }
    let mut fee: u64 = 0;
    for i in 0..n {
        let s = chunk(spends, n, i);
        let o = chunk(payee_outputs, n, i) + extra;
        let actions = u64::from(model.grace_actions.max(s.max(o)));
        fee = fee
            .checked_add(
                model
                    .marginal_fee_zat
                    .checked_mul(actions)
                    .ok_or(PayoutError::Overflow)?,
            )
            .ok_or(PayoutError::Overflow)?;
    }
    Ok((fee, n))
}

/// Size of part `i` when `total` is split into `n` near-equal parts.
fn chunk(total: u32, n: u32, i: u32) -> u32 {
    total / n + u32::from(i < total % n)
}

fn add(a: u64, b: u64) -> Result<u64, PayoutError> {
    a.checked_add(b).ok_or(PayoutError::Overflow)
}

fn sub(a: u64, b: u64) -> Result<u64, PayoutError> {
    a.checked_sub(b).ok_or(PayoutError::Overflow)
}

/// Builds the Payout Plan for one market. Deterministic and independent of note order.
pub fn plan(input: &PlanInput) -> Result<PlanOutcome, PayoutError> {
    let mut notes: Vec<&NoteInput> = input.notes.iter().collect();
    notes.sort_by(|a, b| a.note_ref.cmp(&b.note_ref));
    if let Some(w) = notes.windows(2).find(|w| w[0].note_ref == w[1].note_ref) {
        return Err(PayoutError::DuplicateNote(w[0].note_ref.clone()));
    }

    let mut summary = PlanSummary {
        winners: 0,
        refunds: 0,
        unclaimable: 0,
        swept: 0,
        uneconomic_dust: 0,
        excluded_after_cutoff: 0,
        pool_total_zat: 0,
        distributed_zat: 0,
        operator_fee_zat: 0,
        swept_total_zat: 0,
        fee_budget_zat: 0,
        tx_fee_zat: 0,
        tx_count: 0,
        spends: 0,
    };

    // 1. Select the notes this plan spends.
    let mut included: Vec<&NoteInput> = Vec::new();
    for note in notes {
        if note.block_height > input.plan_cutoff_height {
            summary.excluded_after_cutoff += 1;
            continue;
        }
        match note.status {
            BetStatus::UneconomicDust => summary.uneconomic_dust += 1,
            BetStatus::Pending => return Err(PayoutError::NotFinal(note.note_ref.clone())),
            BetStatus::Counted
            | BetStatus::Late
            | BetStatus::RejectedTransparent
            | BetStatus::IgnoredDust
            | BetStatus::InvalidMemo
            | BetStatus::UnknownOutcome => included.push(note),
            _ if note.is_announcement => included.push(note),
            other => {
                return Err(PayoutError::UnexpectedStatus(note.note_ref.clone(), other));
            }
        }
    }

    // 2. Pools.
    let mut pool_total: u64 = 0;
    let mut winning_total: u64 = 0;
    for note in included
        .iter()
        .filter(|n| n.status == BetStatus::Counted && !n.is_announcement)
    {
        let outcome = note
            .outcome_index
            .ok_or_else(|| PayoutError::MissingOutcome(note.note_ref.clone()))?;
        pool_total = add(pool_total, note.amount_zat)?;
        if input.result == PlanResult::Outcome(outcome) {
            winning_total = add(winning_total, note.amount_zat)?;
        }
    }
    let result = match input.result {
        PlanResult::Outcome(_) if winning_total == 0 => PlanResult::Void,
        r => r,
    };

    let operator_fee_base = match result {
        PlanResult::Void => 0,
        PlanResult::Outcome(_) => {
            u64::try_from(u128::from(pool_total) * u128::from(input.fee_bps) / 10_000)
                .map_err(|_| PayoutError::Overflow)?
        }
    };
    let distributable = sub(pool_total, operator_fee_base)?;

    // 3. Items.
    let mut items: Vec<PlanItem> = Vec::new();
    let mut outputs: Vec<PrivateOutput> = Vec::new();
    let mut swept_total: u64 = 0;
    let mut refund_fees: u64 = 0;
    let mut paid_to_winners: u64 = 0;

    let push_payee = |note: &NoteInput,
                      kind: ItemKind,
                      amount: u64,
                      items: &mut Vec<PlanItem>,
                      outputs: &mut Vec<PrivateOutput>,
                      swept: &mut u64,
                      summary: &mut PlanSummary|
     -> Result<(), PayoutError> {
        match note.envelope {
            EnvelopeResult::Opened { receiver, bet_salt } => {
                items.push(PlanItem {
                    note_ref: note.note_ref.clone(),
                    bet_fingerprint: note.bet_fingerprint.clone(),
                    kind,
                    amount_zat: amount,
                    receiver_commitment: Some(receiver_commitment(&receiver, &bet_salt)),
                });
                outputs.push(PrivateOutput {
                    note_ref: note.note_ref.clone(),
                    kind,
                    receiver,
                    amount_zat: amount,
                });
                match kind {
                    ItemKind::Win => summary.winners += 1,
                    _ => summary.refunds += 1,
                }
            }
            EnvelopeResult::Unopenable | EnvelopeResult::None => {
                items.push(PlanItem {
                    note_ref: note.note_ref.clone(),
                    bet_fingerprint: note.bet_fingerprint.clone(),
                    kind: ItemKind::Unclaimable,
                    amount_zat: amount,
                    receiver_commitment: None,
                });
                *swept = add(*swept, amount)?;
                summary.unclaimable += 1;
            }
        }
        Ok(())
    };

    for note in &included {
        let is_sweep = note.is_announcement
            || matches!(
                note.status,
                BetStatus::IgnoredDust | BetStatus::InvalidMemo | BetStatus::UnknownOutcome
            );
        if is_sweep {
            items.push(PlanItem {
                note_ref: note.note_ref.clone(),
                bet_fingerprint: note.bet_fingerprint.clone(),
                kind: ItemKind::Sweep,
                amount_zat: note.amount_zat,
                receiver_commitment: None,
            });
            swept_total = add(swept_total, note.amount_zat)?;
            summary.swept += 1;
            continue;
        }
        let refund = |amount: u64| {
            sub(amount, input.refund_fee_zat).map_err(|_| {
                PayoutError::Invalid(format!(
                    "note {} is smaller than refund_fee_zat",
                    note.note_ref
                ))
            })
        };
        match (note.status, result) {
            (BetStatus::Late | BetStatus::RejectedTransparent, _)
            | (BetStatus::Counted, PlanResult::Void) => {
                let amount = refund(note.amount_zat)?;
                refund_fees = add(refund_fees, input.refund_fee_zat)?;
                push_payee(
                    note,
                    ItemKind::Refund,
                    amount,
                    &mut items,
                    &mut outputs,
                    &mut swept_total,
                    &mut summary,
                )?;
            }
            (BetStatus::Counted, PlanResult::Outcome(w)) => {
                if note.outcome_index == Some(w) {
                    let amount = u64::try_from(
                        u128::from(note.amount_zat) * u128::from(distributable)
                            / u128::from(winning_total),
                    )
                    .map_err(|_| PayoutError::Overflow)?;
                    paid_to_winners = add(paid_to_winners, amount)?;
                    push_payee(
                        note,
                        ItemKind::Win,
                        amount,
                        &mut items,
                        &mut outputs,
                        &mut swept_total,
                        &mut summary,
                    )?;
                }
                // Losing bets produce no item; their amount is part of `distributable`.
            }
            (other, _) => {
                return Err(PayoutError::UnexpectedStatus(note.note_ref.clone(), other));
            }
        }
    }

    let dust_remainder = match result {
        PlanResult::Void => 0,
        PlanResult::Outcome(_) => sub(distributable, paid_to_winners)?,
    };
    let operator_fee = add(operator_fee_base, dust_remainder)?;

    // 4. Fees (fail closed).
    let spends = u32::try_from(included.len()).map_err(|_| PayoutError::Overflow)?;
    let payee_outputs = u32::try_from(outputs.len()).map_err(|_| PayoutError::Overflow)?;
    let fee_budget = add(add(operator_fee, swept_total)?, refund_fees)?;
    let (fee_with, n_with) = estimate_fee(&input.fee_model, spends, payee_outputs, true)?;
    let (fee_without, n_without) = estimate_fee(&input.fee_model, spends, payee_outputs, false)?;
    // A Treasury output is only worth an action when something is left for it; otherwise the small
    // remainder of the budget is paid as fee (ZIP 317 fees are a minimum). Winners are never charged.
    let (tx_fee, tx_count) = if fee_budget > fee_with {
        (fee_with, n_with)
    } else if fee_budget >= fee_without {
        (fee_budget, n_without)
    } else {
        return Err(PayoutError::FeeShortfall {
            tx_fee_zat: fee_without,
            fee_budget_zat: fee_budget,
        });
    };
    let treasury_out = fee_budget - tx_fee;

    // 5. Value conservation: Σ spent = Σ payee outputs + Treasury + fee.
    let spent: u64 = included
        .iter()
        .try_fold(0u64, |acc, n| add(acc, n.amount_zat))?;
    let paid: u64 = outputs
        .iter()
        .try_fold(0u64, |acc, o| add(acc, o.amount_zat))?;
    if spent != add(add(paid, treasury_out)?, tx_fee)? {
        return Err(PayoutError::Invalid("value is not conserved".into()));
    }

    summary.pool_total_zat = pool_total;
    summary.distributed_zat = distributable;
    summary.operator_fee_zat = operator_fee;
    summary.swept_total_zat = swept_total;
    summary.fee_budget_zat = fee_budget;
    summary.tx_fee_zat = tx_fee;
    summary.tx_count = tx_count;
    summary.spends = spends;

    let mut public = Plan {
        market_id: input.market_id.clone(),
        plan_kind: input.plan_kind,
        plan_cutoff_height: input.plan_cutoff_height,
        result,
        operator_fee_zat: operator_fee,
        items,
    };
    public
        .normalize()
        .map_err(|e| PayoutError::Invalid(e.to_string()))?;

    Ok(PlanOutcome {
        public,
        private: PrivatePlanOutputs {
            outputs,
            treasury_out_zat: treasury_out,
        },
        summary,
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use proptest::prelude::*;

    pub(crate) fn opened(seed: u8) -> EnvelopeResult {
        EnvelopeResult::Opened {
            receiver: [seed; 43],
            bet_salt: [seed.wrapping_add(1); 8],
        }
    }

    pub(crate) fn note(i: u16, status: BetStatus, outcome: Option<u32>, amount: u64) -> NoteInput {
        NoteInput {
            note_ref: format!("{:016x}", u64::from(i) * 0x1000_0001),
            bet_fingerprint: Some(format!("{:016x}", u64::from(i) + 7)),
            outcome_index: outcome,
            amount_zat: amount,
            block_height: 100,
            status,
            is_announcement: false,
            envelope: if matches!(
                status,
                BetStatus::Counted | BetStatus::Late | BetStatus::RejectedTransparent
            ) {
                opened(i as u8)
            } else {
                EnvelopeResult::None
            },
        }
    }

    pub(crate) fn input(result: PlanResult, notes: Vec<NoteInput>) -> PlanInput {
        PlanInput {
            market_id: "a1f3c0de9b7e4f21a0b1c2d3e4f50617".into(),
            plan_kind: PlanKind::Standard,
            plan_cutoff_height: 1_000,
            result,
            fee_bps: 100,
            refund_fee_zat: params::REFUND_FEE_ZAT,
            fee_model: FeeModel::default(),
            notes,
        }
    }

    /// The §22.3 example: YES 770_000_000 over 6 bets, NO 470_000_000 over 3 bets.
    pub(crate) fn example_notes() -> Vec<NoteInput> {
        let mut notes = vec![note(1, BetStatus::Counted, Some(0), 50_000_000)];
        for (i, a) in [
            200_000_000u64,
            120_000_000,
            150_000_000,
            150_000_000,
            100_000_000,
        ]
        .into_iter()
        .enumerate()
        {
            notes.push(note(10 + i as u16, BetStatus::Counted, Some(0), a));
        }
        for (i, a) in [200_000_000u64, 170_000_000, 100_000_000]
            .into_iter()
            .enumerate()
        {
            notes.push(note(20 + i as u16, BetStatus::Counted, Some(1), a));
        }
        notes
    }

    fn item<'a>(p: &'a PlanOutcome, n: &NoteInput) -> Option<&'a PlanItem> {
        p.public.items.iter().find(|i| i.note_ref == n.note_ref)
    }

    #[test]
    fn example_payout() {
        let notes = example_notes();
        let p = plan(&input(PlanResult::Outcome(0), notes.clone())).unwrap();
        assert_eq!(p.summary.pool_total_zat, 1_240_000_000);
        assert_eq!(p.summary.distributed_zat, 1_227_600_000);
        let win = item(&p, &notes[0]).unwrap();
        assert_eq!(win.kind, ItemKind::Win);
        assert_eq!(win.amount_zat, 79_714_285);
        assert!(p.public.operator_fee_zat >= 12_400_000);
        // Losing bets produce no item.
        assert!(item(&p, &notes[6]).is_none());
        assert_eq!(p.summary.winners, 6);
    }

    #[test]
    fn late_bet_refund() {
        let mut notes = example_notes();
        let late = note(30, BetStatus::Late, Some(0), 20_000_000);
        notes.push(late.clone());
        let p = plan(&input(PlanResult::Outcome(0), notes)).unwrap();
        let r = item(&p, &late).unwrap();
        assert_eq!((r.kind, r.amount_zat), (ItemKind::Refund, 19_995_000));
    }

    #[test]
    fn void_when_no_winner() {
        let notes = vec![
            note(1, BetStatus::Counted, Some(1), 30_000_000),
            note(2, BetStatus::Counted, Some(1), 40_000_000),
        ];
        let p = plan(&input(PlanResult::Outcome(0), notes)).unwrap();
        assert_eq!(p.public.result, PlanResult::Void);
        assert_eq!(p.public.operator_fee_zat, 0);
        assert!(p.public.items.iter().all(|i| i.kind == ItemKind::Refund));
        assert_eq!(
            p.public.items[0].amount_zat + p.public.items[1].amount_zat,
            69_990_000
        );
    }

    #[test]
    fn sweeps_unclaimable_and_dust() {
        let mut notes = example_notes();
        let invalid = note(40, BetStatus::InvalidMemo, None, 2_000_000);
        let unknown = note(41, BetStatus::UnknownOutcome, None, 300_000);
        let ignored = note(42, BetStatus::IgnoredDust, Some(0), 500_000);
        let mut announce = note(43, BetStatus::InvalidMemo, Some(0), 10_000);
        announce.is_announcement = true;
        let mut unopenable = note(44, BetStatus::Counted, Some(0), 10_000_000);
        unopenable.envelope = EnvelopeResult::Unopenable;
        notes.extend([
            invalid.clone(),
            unknown.clone(),
            ignored.clone(),
            announce.clone(),
            unopenable.clone(),
        ]);
        for i in 0..50 {
            notes.push(note(100 + i, BetStatus::UneconomicDust, None, 1));
        }
        let p = plan(&input(PlanResult::Outcome(0), notes)).unwrap();
        for n in [&invalid, &unknown, &ignored, &announce] {
            let it = item(&p, n).unwrap();
            assert_eq!(
                (it.kind, it.amount_zat, it.receiver_commitment.clone()),
                (ItemKind::Sweep, n.amount_zat, None)
            );
        }
        let u = item(&p, &unopenable).unwrap();
        assert_eq!(u.kind, ItemKind::Unclaimable);
        assert!(u.receiver_commitment.is_none());
        assert!(u.amount_zat > 0);
        assert_eq!(p.summary.uneconomic_dust, 50);
        assert_eq!(p.summary.spends, 9 + 5);
        // Dust spam does not change any winner's payout.
        let base = plan(&input(PlanResult::Outcome(0), {
            let mut n = example_notes();
            n.push(unopenable.clone());
            n
        }))
        .unwrap();
        assert_eq!(
            item(&base, &example_notes()[0]).unwrap().amount_zat,
            item(&p, &example_notes()[0]).unwrap().amount_zat
        );
    }

    #[test]
    fn notes_after_cutoff_are_excluded() {
        let mut notes = example_notes();
        let mut late = note(30, BetStatus::Late, Some(0), 20_000_000);
        late.block_height = 1_001;
        notes.push(late.clone());
        let p = plan(&input(PlanResult::Outcome(0), notes)).unwrap();
        assert!(item(&p, &late).is_none());
        assert_eq!(p.summary.excluded_after_cutoff, 1);
    }

    #[test]
    fn fee_shortfall_fails_closed() {
        let mut i = input(PlanResult::Outcome(0), example_notes());
        i.fee_bps = 0;
        assert!(matches!(plan(&i), Err(PayoutError::FeeShortfall { .. })));
    }

    /// A VOID market with a single bet cannot pay the 2-action ZIP 317 minimum from one
    /// `REFUND_FEE_ZAT`; planning fails closed and the operator must top up (spec gap, reported).
    #[test]
    fn single_refund_void_market_is_a_shortfall() {
        let notes = vec![note(1, BetStatus::Counted, Some(1), 30_000_000)];
        assert_eq!(
            plan(&input(PlanResult::Outcome(0), notes)),
            Err(PayoutError::FeeShortfall {
                tx_fee_zat: 10_000,
                fee_budget_zat: 5_000
            })
        );
    }

    /// Real markets always hold the FRSTM1 announcement note (`MIN_ANNOUNCE_ZAT`), which is
    /// swept into the fee budget, so a single-bet VOID market still covers the 2-action minimum.
    #[test]
    fn single_refund_void_market_with_announcement_is_covered() {
        let mut announce = note(2, BetStatus::InvalidMemo, Some(0), 10_000);
        announce.is_announcement = true;
        let notes = vec![note(1, BetStatus::Counted, Some(1), 30_000_000), announce];
        let plan = plan(&input(PlanResult::Outcome(0), notes)).expect("fee is covered");
        assert_eq!(plan.public.result, PlanResult::Void);
    }

    #[test]
    fn transactions_are_split() {
        let (fee, n) = estimate_fee(&FeeModel::default(), 120, 60, true).unwrap();
        assert_eq!(n, 3);
        assert_eq!(fee, 3 * 40 * 5_000);
        let (fee, n) = estimate_fee(&FeeModel::default(), 1, 1, true).unwrap();
        assert_eq!((fee, n), (10_000, 1));
        assert_eq!(
            estimate_fee(&FeeModel::default(), 3, 3, false).unwrap(),
            (15_000, 1)
        );
        assert_eq!(
            estimate_fee(&FeeModel::default(), 0, 0, true).unwrap(),
            (0, 0)
        );
        let notes: Vec<_> = (0..120u16)
            .map(|i| note(i, BetStatus::Counted, Some(u32::from(i % 2)), 2_000_000))
            .collect();
        let p = plan(&input(PlanResult::Outcome(0), notes)).unwrap();
        assert!(p.summary.tx_count >= 3);
    }

    #[test]
    fn pending_note_is_rejected() {
        let notes = vec![note(1, BetStatus::Pending, Some(0), 2_000_000)];
        assert!(matches!(
            plan(&input(PlanResult::Outcome(0), notes)),
            Err(PayoutError::NotFinal(_))
        ));
    }

    #[test]
    fn public_plan_has_no_receivers() {
        let p = plan(&input(PlanResult::Outcome(0), example_notes())).unwrap();
        let json = crate::hash::jcs(&serde_json::to_value(&p.public).unwrap()).unwrap();
        for out in &p.private.outputs {
            assert!(!json.contains(&hex::encode(out.receiver)));
        }
        assert!(!json.contains("receiver\""));
    }

    fn arb_status() -> impl Strategy<Value = BetStatus> {
        prop_oneof![
            6 => Just(BetStatus::Counted),
            1 => Just(BetStatus::Late),
            1 => Just(BetStatus::RejectedTransparent),
            1 => Just(BetStatus::IgnoredDust),
            1 => Just(BetStatus::InvalidMemo),
            1 => Just(BetStatus::UnknownOutcome),
            1 => Just(BetStatus::UneconomicDust),
        ]
    }

    fn arb_note() -> impl Strategy<Value = (BetStatus, u32, u64, bool, u32)> {
        (
            arb_status(),
            0u32..3,
            1u64..5_000_000_000,
            any::<bool>(),
            90u32..1_010,
        )
    }

    proptest! {
        #[test]
        fn plan_invariants(
            raw in proptest::collection::vec(arb_note(), 0..80),
            result in prop_oneof![Just(PlanResult::Void), (0u32..3).prop_map(PlanResult::Outcome)],
            fee_bps in 0u32..500,
        ) {
            let notes: Vec<NoteInput> = raw.iter().enumerate().map(|(i, (status, outcome, amount, unopenable, height))| {
                let mut amount = *amount;
                // Respect the §19.5 classification ranges.
                match status {
                    BetStatus::UneconomicDust => amount = amount % 5_000 + 1,
                    BetStatus::IgnoredDust => amount = 5_001 + amount % (1_000_000 - 5_001),
                    _ => amount = amount.max(1_000_000),
                }
                let mut n = note(i as u16, *status, Some(*outcome), amount);
                if matches!(status, BetStatus::InvalidMemo | BetStatus::UnknownOutcome) {
                    amount = amount.max(5_001);
                    n.amount_zat = amount;
                }
                n.block_height = *height;
                if *unopenable && n.envelope != EnvelopeResult::None {
                    n.envelope = EnvelopeResult::Unopenable;
                }
                n
            }).collect();
            let mut i = input(result, notes.clone());
            i.fee_bps = fee_bps;
            match plan(&i) {
                Ok(p) => {
                    let s = &p.summary;
                    let wins: u64 = p.public.items.iter().filter(|x| x.kind == ItemKind::Win).map(|x| x.amount_zat).sum();
                    prop_assert!(wins <= s.distributed_zat);
                    // Determinism and order independence.
                    let mut rev = i.clone();
                    rev.notes.reverse();
                    let p2 = plan(&rev).unwrap();
                    prop_assert_eq!(&p.public, &p2.public);
                    prop_assert_eq!(
                        crate::hash::plan_hash(&p.public).unwrap(),
                        crate::hash::plan_hash(&p2.public).unwrap()
                    );
                    // Winners are never charged fees: payout_i = floor(amount_i × distributable / winning_total).
                    if let PlanResult::Outcome(w) = p.public.result {
                        let winning: u64 = notes.iter()
                            .filter(|n| n.status == BetStatus::Counted && n.outcome_index == Some(w) && n.block_height <= i.plan_cutoff_height)
                            .map(|n| n.amount_zat).sum();
                        for n in notes.iter().filter(|n| n.status == BetStatus::Counted && n.outcome_index == Some(w) && n.block_height <= i.plan_cutoff_height) {
                            let it = p.public.items.iter().find(|x| x.note_ref == n.note_ref).unwrap();
                            let expected = (u128::from(n.amount_zat) * u128::from(s.distributed_zat) / u128::from(winning)) as u64;
                            prop_assert_eq!(it.amount_zat, expected);
                        }
                    }
                }
                Err(PayoutError::FeeShortfall { tx_fee_zat, fee_budget_zat }) => {
                    prop_assert!(tx_fee_zat > fee_budget_zat);
                }
                Err(e) => prop_assert!(false, "unexpected error {e}"),
            }
        }
    }
}

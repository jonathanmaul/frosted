// Labels from docs/PROJECT_SPEC_FROSTED.md §23.1 (market) and §23.2a (bet).
const LABELS: Record<string, string> = {
  ANNOUNCING: "Announcing",
  OPEN: "Open",
  CLOSED: "Closed",
  AWAITING_RESOLUTION: "Awaiting resolution",
  PROPOSED: "Proposed",
  RESOLVED: "Resolved",
  VOID: "Void",
  PAYING: "Paying",
  SETTLED: "Settled",
  WAITING: "Waiting",
  PENDING: "Confirming",
  COUNTED: "Counted",
  WON: "Won",
  PAID: "Paid",
  LOST: "Lost",
  LATE: "Late, refund",
  REJECTED_TRANSPARENT: "Rejected, refund",
  REFUND_QUEUED: "Refund queued",
  REFUNDED: "Refunded",
  IGNORED_DUST: "Below minimum",
  UNECONOMIC_DUST: "Too small",
  UNKNOWN_OUTCOME: "Unknown outcome",
  INVALID_MEMO: "Invalid memo",
  UNCLAIMABLE: "Unclaimable",
  SWEPT: "Swept",
  AMOUNT_MISMATCH: "Amount changed",
  UNAVAILABLE: "Unavailable",
  AMBIGUOUS: "Multiple matches",
};

export function statusLabel(status: string): string {
  return LABELS[status] ?? status;
}

export function StatusBadge({ status }: { status: string }) {
  return (
    <span className="num t-5 inline-block rounded-[var(--radius-1)] border border-[var(--border)] px-2 py-0.5 uppercase tracking-wider text-[var(--ice-muted)]">
      {statusLabel(status)}
    </span>
  );
}

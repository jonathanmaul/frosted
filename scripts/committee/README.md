# Signing a payout on the coordinator host

This MVP uses a trusted dealer and in-process FROST 2-of-3 signing. It is not
distributed DKG or a remote signing service. The operator can load two member
shares on one host; disclose that custody arrangement when demonstrating it.

Prerequisites: the private coordinator and wallet helper are running, the market
is resolved or void, and the escrow wallet has synced. See
[coordinator setup](../../crates/payout-coordinator/README.md). Keep all share
files and member encryption keys on the private host. Never upload them to the
public API, a web page, a chat, or source control.

## 1. Create and review the public plan

```sh
target/debug/frosted-operator payout plan MARKET_ID
target/debug/frosted-operator payout status MARKET_ID
```

Review the complete `plan_hash`, market ID, cutoff height, result, all WIN/REFUND
amounts, receiver commitments, Treasury items, and fee totals. Bettors can check
their own receiver commitments in My Bets. Once M10 is available, also run
`frosted-verify all MARKET_ID` against an independent node.

`FEE_SHORTFALL` creates no plan. `PLAN_ALREADY_EXISTS` prevents a second active
plan. A `VOID` market refunds counted bets under the documented refund rules.

## 2. Approve with two distinct member shares

Use the two actual encrypted share paths created during committee setup and
their corresponding member encryption keys:

```sh
target/debug/frosted-operator payout sign MARKET_ID --plan-hash FULL_PLAN_HASH \
  --share /private/member-one.share.enc --member-key /private/member-one.key \
  --share /private/member-two.share.enc --member-key /private/member-two.key
```

The command displays the public summary and asks each member to type the full
plan hash. Each member must independently compare it with the reviewed plan.
Any mismatch, refusal, or approval timeout aborts signing. Do not approve a hash
copied from an unreviewed prompt. `SIGNING_SESSION_TIMEOUT_SECS` bounds approval
waiting; a retry uses fresh FROST signing nonces.

The operator decrypts shares locally. The coordinator receives signatures, not
shares or member keys. Dummy spends already signed by the IO Finalizer are
skipped. Every real spend requires a valid aggregate signature.

## 3. Watch confirmation

```sh
target/debug/frosted-operator payout status MARKET_ID
```

The coordinator commits the encrypted wallet snapshot and prepared transaction
before broadcast. It retries identical pending bytes after a restart and moves
the plan from BROADCAST to CONFIRMED only after the required confirmations.
S-05 on the market page and `/operator` show the public progress without exposing
operator actions. Payout transaction IDs are public; bettor transaction IDs and
payout receivers are not displayed.

Large batches can need intermediate transactions returning funds to escrow.
Wait for each stage to confirm, then run the same signing command again for the
next stage. Review and approve the same plan hash each time. The estimator
includes intermediate fees; winner amounts stay intact.

If a transaction expires, the coordinator rebuilds with a fresh anchor and the
same plan hash. Two members must approve a fresh signing round. Never create a
second plan to work around an unresolved pending transaction.

## 4. Handle notes received after the cutoff

After the original plan is confirmed:

```sh
target/debug/frosted-operator payout follow-up MARKET_ID
target/debug/frosted-operator payout status MARKET_ID
```

Review the new FOLLOW_UP plan and its own hash, then repeat the signing steps.
It handles newly received late notes; it must not spend the original plan twice
or spend UNECONOMIC_DUST. Verify the final receiver amount against the public
item and retain the public transaction evidence for the demo.

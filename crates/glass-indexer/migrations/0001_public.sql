-- public.db: data anyone can rebuild from the chain plus published UIVKs
-- (docs/PROJECT_SPEC_FROSTED.md §21, table split). Never stores *_enc fields.

CREATE TABLE block (
    height       INTEGER PRIMARY KEY,
    hash         TEXT NOT NULL,
    prev_hash    TEXT NOT NULL,
    processed_at INTEGER NOT NULL
);

CREATE TABLE market (
    market_id              TEXT PRIMARY KEY,           -- 16-byte lowercase hex
    definition_json        TEXT NOT NULL,              -- JCS input of rules_hash
    rules_hash             TEXT NOT NULL,              -- hex
    uivk                   TEXT NOT NULL,
    network                TEXT NOT NULL,
    birthday_height        INTEGER NOT NULL,
    close_height           INTEGER NOT NULL,
    resolve_height         INTEGER NOT NULL,
    min_confirmations      INTEGER NOT NULL,
    announce_timeout_blocks INTEGER NOT NULL,
    status                 TEXT NOT NULL,              -- §23.1
    -- 'indexer' while the status is height-driven (ANNOUNCING/OPEN/CLOSED/AWAITING_RESOLUTION
    -- or VOID by announcement timeout); 'coordinator' once resolution/payout pushes take over.
    status_source          TEXT NOT NULL DEFAULT 'indexer',
    announce_height        INTEGER,
    announce_note_ref      TEXT,
    created_at             INTEGER NOT NULL
);

CREATE TABLE outcome (
    market_id     TEXT NOT NULL REFERENCES market(market_id),
    outcome_index INTEGER NOT NULL,
    label         TEXT NOT NULL,
    address       TEXT NOT NULL,
    PRIMARY KEY (market_id, outcome_index)
);

-- Every decrypted escrow note: bets, non-counted notes, and the announcement note.
CREATE TABLE bet (
    note_ref              TEXT PRIMARY KEY,            -- zcash-glass::hash::note_ref
    bet_id                TEXT NOT NULL UNIQUE,        -- txid:action_index, never exposed publicly
    market_id             TEXT NOT NULL REFERENCES market(market_id),
    outcome_index         INTEGER,
    amount_zat            INTEGER NOT NULL,
    block_height          INTEGER NOT NULL,
    block_hash            TEXT NOT NULL,
    tx_index              INTEGER NOT NULL,
    action_index          INTEGER NOT NULL,
    has_transparent_input INTEGER NOT NULL,
    is_announcement       INTEGER NOT NULL DEFAULT 0,
    status                TEXT NOT NULL,               -- §23.2, or 'ANNOUNCEMENT'
    bet_fingerprint       TEXT,
    tagged                INTEGER NOT NULL DEFAULT 0,
    thaw_tag              BLOB,
    memo_payload          BLOB,                        -- FRST1 payload (173 bytes) when valid
    first_seen_at         INTEGER NOT NULL
);
CREATE INDEX bet_market_height ON bet (market_id, block_height);
CREATE INDEX bet_fingerprint_idx ON bet (bet_fingerprint);

CREATE TABLE persona_registry (
    id              INTEGER PRIMARY KEY CHECK (id = 1),
    uivk            TEXT NOT NULL,
    address         TEXT NOT NULL,
    network         TEXT NOT NULL,
    birthday_height INTEGER NOT NULL
);

CREATE TABLE persona (
    persona_id         TEXT PRIMARY KEY,               -- hex(commitment[0..16])
    commitment         BLOB NOT NULL UNIQUE,
    registered_height  INTEGER NOT NULL,
    registered_note_ref TEXT NOT NULL,
    status             TEXT NOT NULL DEFAULT 'REGISTERED',
    alias              TEXT,
    k_root             BLOB,                           -- only after Thaw (then public)
    thawed_at          INTEGER
);

-- Append-only feed for frosted-api SSE (read by cursor `seq`).
CREATE TABLE event (
    seq          INTEGER PRIMARY KEY AUTOINCREMENT,
    market_id    TEXT NOT NULL,
    kind         TEXT NOT NULL,                        -- odds_updated, bet_counted, ...
    payload_json TEXT NOT NULL,
    block_height INTEGER NOT NULL,
    created_at   INTEGER NOT NULL
);
CREATE INDEX event_market_seq ON event (market_id, seq);

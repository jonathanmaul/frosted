-- Owned by frosted-api; do not alter glass-indexer's SQLx migration history.
CREATE TABLE IF NOT EXISTS api_publication (
    market_id TEXT NOT NULL REFERENCES market(market_id),
    kind TEXT NOT NULL,
    body_json TEXT NOT NULL,
    PRIMARY KEY (market_id, kind)
);

CREATE TABLE IF NOT EXISTS api_ingest_replay (
    market_id TEXT NOT NULL,
    kind TEXT NOT NULL,
    version TEXT NOT NULL, -- hex(SHA256(exact signed request body))
    response_json TEXT NOT NULL,
    PRIMARY KEY (market_id, kind, version)
);
CREATE TABLE IF NOT EXISTS thaw_report (
    persona_id TEXT PRIMARY KEY REFERENCES persona(persona_id) ON DELETE CASCADE,
    report_json TEXT NOT NULL,
    computed_height INTEGER NOT NULL DEFAULT 0
);

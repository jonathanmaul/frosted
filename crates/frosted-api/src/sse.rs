use std::{collections::VecDeque, time::Duration};

use axum::{
    extract::{Path, State},
    http::HeaderMap,
    response::sse::{Event, KeepAlive, Sse},
};
use futures::{Stream, stream};
use serde_json::{Value, json};
use sqlx::Row;

use crate::{
    AppState,
    error::{ApiError, Result},
    public,
};

// Serialize explicit public fields even if an internal event payload gains private metadata.
fn public_payload(kind: &str, payload: Value) -> Option<Value> {
    let fields: &[&str] = match kind {
        "bet_counted" => &["note_ref", "bet_fingerprint"],
        "bet_status_changed" => &["note_ref", "bet_fingerprint", "status", "previous_status"],
        "market_status_changed" => &["status", "previous_status", "from", "to"],
        "payout_updated" => &["plan_hash", "status"],
        "odds_updated" => &["at_height"],
        _ => return None,
    };
    let mut out = json!({});
    for field in fields {
        if let Some(v) = payload.get(*field) {
            out[*field] = v.clone();
        }
    }
    Some(out)
}

pub(crate) async fn events(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Sse<impl Stream<Item = Result<Event>>>> {
    public::market(&state, &id).await?;
    let cursor = headers
        .get("last-event-id")
        .map(|v| {
            v.to_str()
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
                .filter(|n| *n <= i64::MAX as u64)
                .ok_or_else(|| ApiError::bad("Invalid Last-Event-ID"))
        })
        .transpose()?
        .unwrap_or(0);
    let stream = stream::try_unfold(
        (state, id, cursor, VecDeque::new()),
        |(state, id, mut cursor, mut pending): (AppState, String, u64, VecDeque<Event>)| async move {
            loop {
                if let Some(event) = pending.pop_front() {
                    return Ok(Some((event, (state, id, cursor, pending))));
                }
                let rows = sqlx::query("SELECT seq, kind, payload_json FROM event WHERE market_id = ? AND seq > ? ORDER BY seq LIMIT 100")
                .bind(&id).bind(cursor as i64).fetch_all(&state.read).await?;
                if rows.is_empty() {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    continue;
                }
                for row in rows {
                    cursor = row.get::<i64, _>(0) as u64;
                    let kind: String = row.get(1);
                    let payload: Value = serde_json::from_str(row.get(2))?;
                    if let Some(payload) = public_payload(&kind, payload) {
                        let event = Event::default()
                            .id(cursor.to_string())
                            .event(kind)
                            .json_data(payload)
                            .map_err(|_| ApiError::bad("Invalid event payload"))?;
                        pending.push_back(event);
                    }
                }
            }
        },
    );
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

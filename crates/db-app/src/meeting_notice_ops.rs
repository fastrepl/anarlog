//! This device's claims on the meeting chat notice: one notice per account and session, written
//! before the notice can be sent and kept through crashes and restarts. Local-only by design.

use std::fmt::Write;

use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

/// How long a claim holds: a later recording of the same session within this time never posts its
/// notice again, even after a restart.
pub const MEETING_NOTICE_CLAIM_RETENTION_MS: i64 = 7 * 24 * 60 * 60 * 1000;

/// How a claimed notice ended when it was, or may have been, sent. Such a claim is never given
/// back; only a notice that was definitely not sent releases its claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeetingNoticeSettlement {
    Sent,
    Unconfirmed,
}

impl MeetingNoticeSettlement {
    fn as_str(self) -> &'static str {
        match self {
            MeetingNoticeSettlement::Sent => "sent",
            MeetingNoticeSettlement::Unconfirmed => "unconfirmed",
        }
    }
}

/// The stored key of one account's claim on one session; neither id is stored.
pub fn meeting_notice_claim_key(account_id: &str, session_id: &str) -> String {
    Sha256::digest(format!("{account_id}\n{session_id}").as_bytes())
        .iter()
        .fold(String::with_capacity(64), |mut output, byte| {
            write!(output, "{byte:02x}").expect("writing to String cannot fail");
            output
        })
}

/// Claims the account's notice for the session before it is sent. False when an unexpired claim
/// already holds it. Claims older than the retention are dropped first; a claim dated after `now_ms`
/// (the clock went back) is kept. Any storage error fails the claim, so nothing is sent without one.
pub async fn claim_meeting_notice(
    pool: &SqlitePool,
    account_id: &str,
    session_id: &str,
    capture_id: &str,
    now_ms: i64,
) -> Result<bool, sqlx::Error> {
    let claim_key = meeting_notice_claim_key(account_id, session_id);
    let mut transaction = pool.begin().await?;
    sqlx::query("DELETE FROM local_meeting_notice_claims WHERE ? - claimed_at_ms > ?")
        .bind(now_ms)
        .bind(MEETING_NOTICE_CLAIM_RETENTION_MS)
        .execute(&mut *transaction)
        .await?;
    let inserted = sqlx::query(
        "INSERT INTO local_meeting_notice_claims (claim_key, capture_id, claimed_at_ms)
         VALUES (?, ?, ?)
         ON CONFLICT(claim_key) DO NOTHING",
    )
    .bind(claim_key)
    .bind(capture_id)
    .bind(now_ms)
    .execute(&mut *transaction)
    .await?
    .rows_affected()
        == 1;
    transaction.commit().await?;
    Ok(inserted)
}

/// Records that the capture's claimed notice was or may have been sent. Only the capture that
/// holds a claim still being dispatched can settle it.
pub async fn settle_meeting_notice(
    pool: &SqlitePool,
    account_id: &str,
    session_id: &str,
    capture_id: &str,
    settlement: MeetingNoticeSettlement,
    now_ms: i64,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "UPDATE local_meeting_notice_claims
         SET state = ?, settled_at_ms = ?
         WHERE claim_key = ? AND capture_id = ? AND state = 'dispatching'",
    )
    .bind(settlement.as_str())
    .bind(now_ms)
    .bind(meeting_notice_claim_key(account_id, session_id))
    .bind(capture_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// Gives back the capture's claim on a notice that was definitely not sent, so a later recording
/// of the session may post. A sent or unconfirmed claim, or another capture's, is never released.
pub async fn release_meeting_notice(
    pool: &SqlitePool,
    account_id: &str,
    session_id: &str,
    capture_id: &str,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "DELETE FROM local_meeting_notice_claims
         WHERE claim_key = ? AND capture_id = ? AND state = 'dispatching'",
    )
    .bind(meeting_notice_claim_key(account_id, session_id))
    .bind(capture_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// One notice delivery outcome, as local evidence for the session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisclosureDelivery {
    pub id: String,
    pub session_id: String,
    pub capture_id: String,
    pub attempted_at_ms: i64,
    /// One of `slack_huddle`, `zoom`, `google_meet`, `teams`, `webex`, `browser`, `unknown`.
    pub platform: String,
    pub message_version: String,
    pub message: String,
    /// One of `sent`, `not_sent`, `cancelled`, `unconfirmed`.
    pub delivery: String,
    pub reason: String,
}

pub async fn insert_disclosure_delivery(
    pool: &SqlitePool,
    delivery: &DisclosureDelivery,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO session_disclosure_deliveries (
           id, session_id, capture_id, attempted_at, platform,
           message_version, message, delivery, reason
         ) VALUES (
           ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', ? / 1000.0, 'unixepoch'),
           ?, ?, ?, ?, ?
         )",
    )
    .bind(&delivery.id)
    .bind(&delivery.session_id)
    .bind(&delivery.capture_id)
    .bind(delivery.attempted_at_ms)
    .bind(&delivery.platform)
    .bind(&delivery.message_version)
    .bind(&delivery.message)
    .bind(&delivery.delivery)
    .bind(&delivery.reason)
    .execute(pool)
    .await?;
    Ok(())
}

import type { DisclosureAttempt, ParticipantConsent } from "./meeting-consent";

import { executeTransaction } from "~/db";

export async function persistDisclosureAttempt(
  attempt: DisclosureAttempt,
): Promise<void> {
  await executeTransaction([
    {
      sql: `
        INSERT INTO session_disclosure_attempts (
          id, session_id, attempted_at, platform, surface,
          message_version, message, delivery, failure_reason
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
      `,
      params: [
        attempt.id,
        attempt.sessionId,
        attempt.attemptedAt,
        attempt.platform,
        attempt.surface,
        attempt.messageVersion,
        attempt.message,
        attempt.delivery,
        attempt.failureReason,
      ],
    },
  ]);
}

export async function persistParticipantConsent(
  consent: ParticipantConsent,
): Promise<void> {
  await executeTransaction([
    {
      sql: `
        INSERT INTO session_participant_consent (
          session_id, participant_key, status, source, updated_at
        ) VALUES (?, ?, ?, ?, ?)
        ON CONFLICT(session_id, participant_key) DO UPDATE SET
          status = excluded.status,
          source = excluded.source,
          updated_at = excluded.updated_at
      `,
      params: [
        consent.sessionId,
        consent.participantKey,
        consent.status,
        consent.source,
        consent.updatedAt,
      ],
    },
  ]);
}

import { useQuery } from "@tanstack/react-query";

import { md2json } from "@anlg/editor/markdown";

import { executeTransaction, liveQueryClient } from "~/db";
import { enqueueDatabaseWrite } from "~/db/write-queue";
import { updateEnhancedNoteContent } from "~/session/queries/enhanced-notes";
import { updateSession } from "~/session/queries/sessions";

export type SessionProposalRecord = {
  id: string;
  sessionId: string;
  kind: "summary_replace" | "memo_replace" | string;
  targetId: string;
  baseUpdatedAt: string;
  currentMarkdown: string;
  proposedMarkdown: string;
  status: "pending" | "applied" | "declined" | string;
  source: string;
  createdAt: string;
  updatedAt: string;
};

type ProposalSqlRow = {
  id: string;
  session_id: string;
  kind: string;
  target_id: string;
  base_updated_at: string;
  current_markdown: string;
  proposed_markdown: string;
  status: string;
  source: string;
  created_at: string;
  updated_at: string;
};

const PROPOSAL_COLUMNS = `
  SELECT
    id,
    session_id,
    kind,
    target_id,
    base_updated_at,
    current_markdown,
    proposed_markdown,
    status,
    source,
    created_at,
    updated_at
  FROM session_proposals
`;

export async function insertSessionProposal(input: {
  id: string;
  sessionId: string;
  kind: "summary_replace" | "memo_replace";
  targetId: string;
  baseUpdatedAt: string;
  currentMarkdown: string;
  proposedMarkdown: string;
  source: string;
}): Promise<void> {
  await enqueueDatabaseWrite(`session:${input.sessionId}`, async () => {
    await executeTransaction([
      {
        sql: `
          INSERT INTO session_proposals (
            id, session_id, kind, target_id, base_updated_at,
            current_markdown, proposed_markdown, status, source
          ) VALUES (?, ?, ?, ?, ?, ?, ?, 'pending', ?)
        `,
        params: [
          input.id,
          input.sessionId,
          input.kind,
          input.targetId,
          input.baseUpdatedAt,
          input.currentMarkdown,
          input.proposedMarkdown,
          input.source,
        ],
      },
    ]);
  });
}

export async function loadSessionProposal(
  proposalId: string,
): Promise<SessionProposalRecord | null> {
  const rows = await liveQueryClient.execute<ProposalSqlRow>(
    `${PROPOSAL_COLUMNS}
     WHERE id = ?
     LIMIT 1`,
    [proposalId],
  );
  return rows[0] ? mapProposal(rows[0]) : null;
}

export async function loadPendingSessionProposals(
  sessionId: string,
): Promise<SessionProposalRecord[]> {
  const rows = await liveQueryClient.execute<ProposalSqlRow>(
    `${PROPOSAL_COLUMNS}
     WHERE session_id = ? AND status = 'pending'
     ORDER BY created_at DESC, id DESC`,
    [sessionId],
  );
  return rows.map(mapProposal);
}

export function sessionProposalsQueryKey(sessionId: string) {
  return ["session-proposals", sessionId] as const;
}

export function usePendingSessionProposals(
  sessionId: string,
): SessionProposalRecord[] {
  const { data = [] } = useQuery({
    queryKey: sessionProposalsQueryKey(sessionId),
    queryFn: () => loadPendingSessionProposals(sessionId),
    enabled: Boolean(sessionId),
    refetchOnWindowFocus: true,
    refetchInterval: 4000,
  });
  return sessionId ? data : [];
}

export async function persistChatSessionProposal(input: {
  id: string;
  sessionId: string;
  kind: "summary_replace" | "memo_replace";
  targetId: string;
  currentMarkdown: string;
  proposedMarkdown: string;
}): Promise<void> {
  const baseUpdatedAt =
    (await loadTargetUpdatedAt({
      targetId: input.targetId,
      sessionId: input.sessionId,
    })) ?? "";
  await insertSessionProposal({
    ...input,
    baseUpdatedAt,
    source: "chat",
  });
}

export async function applySessionProposal(proposalId: string): Promise<void> {
  const proposal = await loadSessionProposal(proposalId);
  if (!proposal) {
    throw new Error("Proposal not found");
  }
  if (proposal.status === "applied") {
    return;
  }
  if (proposal.status !== "pending") {
    throw new Error(`Proposal is ${proposal.status}`);
  }

  const currentUpdatedAt = await loadTargetUpdatedAt(proposal);
  if (currentUpdatedAt && currentUpdatedAt !== proposal.baseUpdatedAt) {
    throw new Error(
      "This proposal is stale. The meeting changed after it was created.",
    );
  }

  const json = JSON.stringify(md2json(proposal.proposedMarkdown));
  if (proposal.kind === "memo_replace") {
    await updateSession(proposal.sessionId, { raw_md: json });
  } else {
    await updateEnhancedNoteContent(
      proposal.targetId,
      proposal.sessionId,
      json,
    );
  }
  await setProposalStatus(proposal.id, proposal.sessionId, "applied");
}

export async function declineSessionProposal(
  proposalId: string,
): Promise<void> {
  const proposal = await loadSessionProposal(proposalId);
  if (!proposal || proposal.status !== "pending") {
    return;
  }
  await setProposalStatus(proposal.id, proposal.sessionId, "declined");
}

async function setProposalStatus(
  proposalId: string,
  sessionId: string,
  status: "applied" | "declined",
): Promise<void> {
  await enqueueDatabaseWrite(`session:${sessionId}`, async () => {
    const now = new Date().toISOString();
    await executeTransaction([
      {
        sql: `
          UPDATE session_proposals
          SET status = ?, updated_at = ?
          WHERE id = ? AND status = 'pending'
        `,
        params: [status, now, proposalId],
      },
    ]);
  });
}

async function loadTargetUpdatedAt(proposal: {
  targetId: string;
  sessionId: string;
}): Promise<string | null> {
  const rows = await liveQueryClient.execute<{ updated_at: string }>(
    `
      SELECT updated_at
      FROM session_documents
      WHERE id = ?
        AND session_id = ?
        AND deleted_at IS NULL
      LIMIT 1
    `,
    [proposal.targetId || proposal.sessionId, proposal.sessionId],
  );
  return rows[0]?.updated_at ?? null;
}

function mapProposal(row: ProposalSqlRow): SessionProposalRecord {
  return {
    id: row.id,
    sessionId: row.session_id,
    kind: row.kind,
    targetId: row.target_id,
    baseUpdatedAt: row.base_updated_at,
    currentMarkdown: row.current_markdown,
    proposedMarkdown: row.proposed_markdown,
    status: row.status,
    source: row.source,
    createdAt: row.created_at,
    updatedAt: row.updated_at,
  };
}

export type SessionIngestEnvelope = Record<string, unknown> & {
  schema_version: number;
  source_id: string;
  revision: number;
  finalized: boolean;
  workspace_id: string;
  session: Record<string, unknown> & { id: string; status: string };
};

export type DeliveryItem = {
  cursor: number;
  jobId: string;
  revision: number;
  finalized: boolean;
  contentHash: string;
  acknowledged: boolean;
  createdAt: string;
  envelope: SessionIngestEnvelope;
};

export type DeliveryPage = {
  items: DeliveryItem[];
  nextCursor: number;
  hasMore: boolean;
};

export type PendingAcknowledgement = {
  jobId: string;
  revision: number;
  contentHash: string;
};

export type PendingCompletion = {
  sourceId: string;
  workspaceId: string;
  sessionId: string;
  revision: number;
};

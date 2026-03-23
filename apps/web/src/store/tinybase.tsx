import * as _UI from "tinybase/ui-react/with-schemas";
import { type Store as _Store, createStore } from "tinybase/with-schemas";

import type { JSONContent } from "@hypr/tiptap/editor";

export const STORE_ID = "file-transcription";

const TABLE_SCHEMA = {
  sessions: {
    title: { type: "string" },
    file_name: { type: "string" },
    transcript: { type: "string" },
    transcript_segments: { type: "string" },
    summary: { type: "string" },
    memo: { type: "string" },
    pipeline_status: { type: "string" },
    upload_progress: { type: "number" },
    error_message: { type: "string" },
    is_summarizing: { type: "boolean" },
    summary_error: { type: "string" },
    pipeline_id: { type: "string" },
  },
} as const;

const VALUE_SCHEMA = {} as const;

type Schemas = [typeof TABLE_SCHEMA, typeof VALUE_SCHEMA];

export type SessionCell =
  | "title"
  | "file_name"
  | "transcript"
  | "transcript_segments"
  | "summary"
  | "memo"
  | "pipeline_status"
  | "upload_progress"
  | "error_message"
  | "is_summarizing"
  | "summary_error"
  | "pipeline_id";

const { useCreateStore, useProvideStore } = _UI as _UI.WithSchemas<Schemas>;

export const UI = _UI as _UI.WithSchemas<Schemas>;
export type Store = _Store<Schemas>;

const DEFAULT_MEMO: JSONContent = {
  type: "doc",
  content: [
    {
      type: "paragraph",
      content: [{ type: "text", text: "Take notes while transcribing..." }],
    },
  ],
};

export function createSessionRow(store: Store, sessionId: string) {
  store.setRow("sessions", sessionId, {
    title: "",
    file_name: "",
    transcript: "",
    transcript_segments: "",
    summary: "",
    memo: JSON.stringify(DEFAULT_MEMO),
    pipeline_status: "idle",
    upload_progress: 0,
    error_message: "",
    is_summarizing: false,
    summary_error: "",
    pipeline_id: "",
  });
}

export function StoreProvider({ children }: { children: React.ReactNode }) {
  const store = useCreateStore(() =>
    createStore().setTablesSchema(TABLE_SCHEMA).setValuesSchema(VALUE_SCHEMA),
  );

  useProvideStore(STORE_ID, store);

  return <>{children}</>;
}

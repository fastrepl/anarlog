import { PostgresSaver } from "@langchain/langgraph-checkpoint-postgres";

import { env } from "../env";

let checkpointerInstance: PostgresSaver | null = null;

export function createCheckpointer(): PostgresSaver {
  if (!checkpointerInstance) {
    checkpointerInstance = PostgresSaver.fromConnString(env.DATABASE_URL);
  }
  return checkpointerInstance;
}

export const checkpointer = createCheckpointer();

export async function setupCheckpointer(): Promise<void> {
  await checkpointer.setup();
}

export async function clearThread(threadId: string): Promise<void> {
  await checkpointer.deleteThread(threadId);
}

export function generateRunId(): string {
  return crypto.randomUUID();
}

export function getLangSmithUrl(threadId: string): string | null {
  if (!env.LANGSMITH_API_KEY || !env.LANGSMITH_ORG_ID) return null;
  return `https://smith.langchain.com/o/${env.LANGSMITH_ORG_ID}/projects/p/${env.LANGSMITH_PROJECT}?peekedConversationId=${threadId}`;
}

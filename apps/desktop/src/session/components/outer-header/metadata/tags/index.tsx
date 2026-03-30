import { TagInput } from "./input";

export function TagsDisplay({ sessionId }: { sessionId: string }) {
  return <TagInput sessionId={sessionId} />;
}

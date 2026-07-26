import { useAITask } from "~/ai/contexts";
import { createTaskId } from "~/store/zustand/ai-task/task-configs";
import { getTaskState } from "~/store/zustand/ai-task/tasks";

export function useTitleGenerating(sessionId: string): boolean {
  const titleTaskId = createTaskId(sessionId, "title");

  const isGenerating = useAITask((state) => {
    const taskState = getTaskState(state.tasks, titleTaskId);
    return taskState?.status === "generating";
  });

  return isGenerating;
}

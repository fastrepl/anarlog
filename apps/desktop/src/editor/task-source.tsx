import { createContext, useContext, type ReactNode } from "react";

import type { TaskSource } from "./tasks";

const TaskSourceContext = createContext<TaskSource | null>(null);

export function TaskSourceProvider({
  source,
  children,
}: {
  source: TaskSource | null;
  children: ReactNode;
}) {
  return (
    <TaskSourceContext.Provider value={source}>
      {children}
    </TaskSourceContext.Provider>
  );
}

export function useTaskSourceOptional(): TaskSource | null {
  return useContext(TaskSourceContext);
}

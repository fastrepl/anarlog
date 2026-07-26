import React, { createContext, useContext, useMemo, useRef } from "react";
import { shallow } from "zustand/shallow";
import { useStoreWithEqualityFn } from "zustand/traditional";

import { type ToolScope, useRegisterTools } from "~/contexts/tool";
import { type AITaskStore, createAITaskStore } from "~/store/zustand/ai-task";

const AITaskContext = createContext<AITaskStore | null>(null);

export type AITaskState = ReturnType<
  ReturnType<typeof createAITaskStore>["getState"]
>;

export const AITaskProvider = ({
  children,
  store,
  tools,
}: {
  children: React.ReactNode;
  store: AITaskStore;
  tools?: Record<string, any> | ((scope: ToolScope) => Record<string, any>);
}) => {
  const storeRef = useRef<AITaskStore | null>(null);
  if (!storeRef.current) {
    storeRef.current = store;
  }

  const resolvedTools = useMemo(() => {
    if (!tools) {
      return null;
    }

    if (typeof tools === "function") {
      return tools("enhancing");
    }

    return tools;
  }, [tools]);

  useRegisterTools("enhancing", () => resolvedTools ?? {}, [resolvedTools]);

  return (
    <AITaskContext.Provider value={storeRef.current}>
      {children}
    </AITaskContext.Provider>
  );
};

export const useAITask = <T,>(
  selector: (state: AITaskState) => T,
  equalityFn?: (left: T, right: T) => boolean,
) => {
  const store = useContext(AITaskContext);

  if (!store) {
    throw new Error("'useAITask' must be used within a 'AITaskProvider'");
  }

  return useStoreWithEqualityFn(store, selector, equalityFn ?? shallow);
};

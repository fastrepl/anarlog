import { createContext, type ReactNode, useContext } from "react";

type ElicitationState = {
  pending: { message: string } | null;
  respond: ((approved: boolean) => void) | null;
};

const ElicitationContext = createContext<ElicitationState>({
  pending: null,
  respond: null,
});

export function ElicitationProvider({
  pending,
  respond,
  children,
}: {
  pending: { message: string } | null;
  respond: ((approved: boolean) => void) | null;
  children: ReactNode;
}) {
  return (
    <ElicitationContext.Provider value={{ pending, respond }}>
      {children}
    </ElicitationContext.Provider>
  );
}

export function useElicitation() {
  return useContext(ElicitationContext);
}

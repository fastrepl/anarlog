import { createContext, useContext } from "react";

export type LinkedItemOpenBehavior = "current" | "new";

export const LinkedItemOpenBehaviorContext =
  createContext<LinkedItemOpenBehavior>("current");

export function useLinkedItemOpenBehavior() {
  return useContext(LinkedItemOpenBehaviorContext);
}

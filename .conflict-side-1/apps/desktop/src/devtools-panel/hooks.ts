import { useOwnerUserId } from "~/shared/owner-user";

export function useDevtoolsUserId() {
  return useOwnerUserId() ?? undefined;
}

import { useQuery } from "@tanstack/react-query";

import type { AvailableShareWorkspace } from "./source";

import { useAuth } from "~/auth";
import {
  getWorkspacePolicy,
  intersectAllowedShareScopes,
  requireTeamContext,
  type WorkspacePolicy,
} from "~/settings/team/client";

const DEFAULT_SCOPES: WorkspacePolicy["allowedShareScopes"] = [
  "restricted",
  "workspace",
  "link",
  "public",
];

export function useWorkspaceShareScopes(
  workspaces: AvailableShareWorkspace[],
): WorkspacePolicy["allowedShareScopes"] {
  const auth = useAuth();
  const workspaceIds = workspaces.map((workspace) => workspace.id).join(",");
  const { data = DEFAULT_SCOPES } = useQuery({
    queryKey: ["workspace-share-scopes", workspaceIds],
    enabled: Boolean(auth.session && auth.supabase && workspaces.length > 0),
    retry: false,
    queryFn: async () => {
      const context = requireTeamContext(auth);
      const policies = await Promise.all(
        workspaces.map((workspace) =>
          getWorkspacePolicy(context, workspace.id).catch(() => ({
            allowedShareScopes: DEFAULT_SCOPES,
          })),
        ),
      );
      return intersectAllowedShareScopes(policies);
    },
  });
  return data;
}

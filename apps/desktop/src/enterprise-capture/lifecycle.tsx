import { useQuery } from "@tanstack/react-query";
import { useRef } from "react";

import { commands as miscCommands } from "@anlg/plugin-misc";

import {
  dispatchPendingEnterpriseCompletions,
  syncEnterpriseWorkspace,
} from "./sync";

import { useAuth } from "~/auth";
import { env } from "~/env";
import { useMyWorkspacesWithMirror } from "~/settings/team/mirror";

const POLL_INTERVAL_MS = 15_000;

export function EnterpriseCaptureSync() {
  const auth = useAuth();
  const workspaces = useMyWorkspacesWithMirror();
  const serverUrl = env.VITE_ENTERPRISE_API_URL;
  const session = auth.session;
  const deviceFingerprint = useRef<Promise<string> | null>(null);

  useQuery({
    queryKey: [
      "enterprise-capture-delivery",
      serverUrl,
      session?.user.id,
      workspaces.data?.map((workspace) => workspace.workspaceId).sort(),
    ],
    enabled: Boolean(serverUrl && session && workspaces.data),
    queryFn: async () => {
      if (!serverUrl || !session || !workspaces.data) return null;
      const consumerId = await getDeviceFingerprint(deviceFingerprint);
      let workspaceError: unknown;
      for (const workspace of workspaces.data) {
        try {
          await syncEnterpriseWorkspace({
            serverUrl,
            accessToken: session.access_token,
            workspaceId: workspace.workspaceId,
            consumerId,
          });
        } catch (error) {
          workspaceError ??= error;
        }
      }
      await dispatchPendingEnterpriseCompletions();
      if (workspaceError) throw workspaceError;
      return null;
    },
    refetchInterval: POLL_INTERVAL_MS,
    refetchIntervalInBackground: true,
    refetchOnWindowFocus: true,
    retry: 2,
  });

  return null;
}

async function getDeviceFingerprint(cache: {
  current: Promise<string> | null;
}): Promise<string> {
  cache.current ??= (async () => {
    const result = await miscCommands.getFingerprint();
    if (result.status === "error") throw new Error(result.error);
    if (!result.data) throw new Error("device fingerprint is unavailable");
    return result.data;
  })();
  try {
    return await cache.current;
  } catch (error) {
    cache.current = null;
    throw error;
  }
}

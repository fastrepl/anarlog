import type { Session } from "@supabase/supabase-js";
import { useQueryClient } from "@tanstack/react-query";

import { createAttachmentBackupClient } from "./client";
import { startAttachmentTransferRunner } from "./runner";

import { useAuth } from "~/auth";
import { useBillingAccess } from "~/auth/billing-context";
import { env } from "~/env";
import { sessionAttachmentPathsQueryKey } from "~/session/hooks/useAttachmentResolver";
import { useConfigValue } from "~/shared/config";
import { useLatestRef } from "~/shared/hooks/useLatestRef";
import { useMountEffect } from "~/shared/hooks/useMountEffect";

export function AttachmentTransferLifecycle() {
  const auth = useAuth();
  const billing = useBillingAccess();
  const cloudSyncEnabled = useConfigValue("cloud_sync_enabled");
  const session = auth.session;
  const supabaseUrl = env.VITE_SUPABASE_URL;

  if (
    !session ||
    session.user.is_anonymous === true ||
    !billing.isPaid ||
    !cloudSyncEnabled ||
    !supabaseUrl
  ) {
    return null;
  }

  return (
    <ActiveAttachmentTransferLifecycle
      key={session.user.id}
      session={session}
      supabaseUrl={supabaseUrl}
    />
  );
}

function ActiveAttachmentTransferLifecycle({
  session,
  supabaseUrl,
}: {
  session: Session;
  supabaseUrl: string;
}) {
  const queryClient = useQueryClient();
  const accessTokenRef = useLatestRef(session.access_token);

  useMountEffect(() =>
    startAttachmentTransferRunner({
      client: createAttachmentBackupClient({
        apiBaseUrl: env.VITE_API_URL,
        getAccessToken: () => accessTokenRef.current,
      }),
      supabaseUrl,
      onAttachmentRestored: ({ sessionId }) => {
        void queryClient.invalidateQueries({
          queryKey: sessionAttachmentPathsQueryKey(sessionId),
        });
      },
    }),
  );

  return null;
}

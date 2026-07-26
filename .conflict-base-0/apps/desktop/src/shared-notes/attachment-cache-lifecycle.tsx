import type { Session } from "@supabase/supabase-js";
import { useRef } from "react";

import { startSharedAttachmentCacheRunner } from "./attachment-cache-runner";
import { createSharedAttachmentClient } from "./attachment-client";

import { useAuth } from "~/auth";
import { env } from "~/env";
import { useMountEffect } from "~/shared/hooks/useMountEffect";

export function SharedAttachmentCacheLifecycle() {
  const { session } = useAuth();
  if (!session || session.user.is_anonymous === true) {
    return null;
  }
  return (
    <ActiveSharedAttachmentCacheLifecycle
      key={session.user.id}
      session={session}
    />
  );
}

function ActiveSharedAttachmentCacheLifecycle({
  session,
}: {
  session: Session;
}) {
  const sessionRef = useRef(session);
  sessionRef.current = session;
  useMountEffect(() =>
    startSharedAttachmentCacheRunner({
      viewerUserId: session.user.id,
      client: {
        download: (shareId, attachmentId, signal) =>
          createSharedAttachmentClient({
            apiBaseUrl: env.VITE_API_URL,
            session: sessionRef.current,
          }).download(shareId, attachmentId, signal),
      },
    }),
  );
  return null;
}

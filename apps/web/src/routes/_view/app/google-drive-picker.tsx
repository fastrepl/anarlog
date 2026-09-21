import { useMutation } from "@tanstack/react-query";
import { createFileRoute } from "@tanstack/react-router";
import { useEffect, useRef, useState } from "react";

import { consumeDrivePickerHandoff } from "@/lib/drive-picker";
import { returnToDesktop } from "@/lib/integration-desktop-return";

export const Route = createFileRoute("/_view/app/google-drive-picker")({
  component: DrivePickerPage,
  head: () => ({
    meta: [
      { name: "robots", content: "noindex, nofollow" },
      { name: "referrer", content: "no-referrer" },
    ],
  }),
});

function DrivePickerPage() {
  const [handoff, setHandoff] =
    useState<ReturnType<typeof consumeDrivePickerHandoff>>(null);
  const captured = useRef(false);
  const delivery = useMutation({
    mutationFn: ({ callback, port }: { callback: string; port?: number }) =>
      returnToDesktop(callback, port),
  });
  const deliver = delivery.mutate;
  useEffect(() => {
    if (captured.current) return;
    captured.current = true;
    const result = consumeDrivePickerHandoff();
    setHandoff(result);
    if (result?.callback)
      deliver({ callback: result.callback, port: result.port });
  }, [deliver]);
  return (
    <main className="bg-page text-fg flex min-h-screen flex-col items-center justify-center gap-4 p-8">
      <h1 className="font-mono text-xl">Choose a Google Drive folder</h1>
      {handoff?.authorizationUrl ? (
        <>
          <p className="text-fg-muted">
            Google will open in this tab. Use the same Google account you
            connected to Anarlog.
          </p>
          <button
            className="rounded-full border px-5 py-2 font-mono"
            onClick={() => window.location.assign(handoff.authorizationUrl!)}
          >
            Continue to Google
          </button>
        </>
      ) : handoff?.callback ? (
        <>
          <p className="text-fg-muted">
            Return to Anarlog to finish validating your folder.
          </p>
          <button
            className="rounded-full border px-5 py-2 font-mono"
            disabled={delivery.isPending}
            onClick={() =>
              deliver({ callback: handoff.callback!, port: handoff.port })
            }
          >
            Return to Anarlog
          </button>
        </>
      ) : (
        <p>Return to Anarlog and choose a folder again.</p>
      )}
      {delivery.error ? (
        <p role="alert">
          Could not reach Anarlog. Return to the app and choose a folder again.
        </p>
      ) : null}
    </main>
  );
}

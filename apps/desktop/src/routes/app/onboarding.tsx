import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useCallback } from "react";

import { resolveShellEntryPath } from "./-resolve-entry-path";

import { StandaloneOnboardingScreen } from "~/onboarding";
import { useTabs } from "~/store/zustand/tabs";

export const Route = createFileRoute("/app/onboarding")({
  component: Component,
});

function Component() {
  const navigate = useNavigate();
  const openCurrent = useTabs((state) => state.openCurrent);

  const handleFinish = useCallback(
    (sessionId: string) => {
      openCurrent({ type: "sessions", id: sessionId });
      void (async () => {
        await navigate({ to: await resolveShellEntryPath() });
      })();
    },
    [navigate, openCurrent],
  );

  return <StandaloneOnboardingScreen onFinish={handleFinish} />;
}

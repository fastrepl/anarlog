import { createFileRoute, useNavigate } from "@tanstack/react-router";
import { useCallback } from "react";

import { resolveShellEntryPath } from "./-resolve-entry-path";

import { StandaloneOnboardingScreen } from "~/onboarding";

export const Route = createFileRoute("/app/onboarding")({
  component: Component,
});

function Component() {
  const navigate = useNavigate();

  const handleFinish = useCallback(() => {
    void (async () => {
      await navigate({ to: await resolveShellEntryPath() });
    })();
  }, [navigate]);

  return <StandaloneOnboardingScreen onFinish={handleFinish} />;
}

import { createFileRoute } from "@tanstack/react-router";
import { useCallback } from "react";

import { dismissInstruction } from "@hypr/plugin-windows";

import { useAuth } from "~/auth";
import { InstructionScreen, type InstructionType } from "~/instruction";

export const Route = createFileRoute("/app/instruction")({
  validateSearch: (
    search,
  ): { type: InstructionType; url?: string; integrationId?: string } => ({
    type: ((search as { type?: string }).type ?? "sign-in") as InstructionType,
    url: (search as { url?: string }).url,
    integrationId: (search as { integrationId?: string }).integrationId,
  }),
  component: InstructionRoute,
});

function useHandleBack() {
  return useCallback(() => dismissInstruction(), []);
}

function InstructionRoute() {
  const auth = useAuth();
  const { type, url, integrationId } = Route.useSearch();
  const handleBack = useHandleBack();
  const onBack = useCallback(() => void handleBack(), [handleBack]);
  const onCleanup = useCallback(() => {
    if (type === "billing") {
      void auth.refreshSession();
    }
  }, [auth, type]);

  return (
    <InstructionScreen
      type={type}
      url={url}
      integrationId={integrationId}
      onBack={onBack}
      onCleanup={onCleanup}
    />
  );
}

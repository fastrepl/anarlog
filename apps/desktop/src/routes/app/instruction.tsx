import { createFileRoute } from "@tanstack/react-router";
import { useCallback } from "react";

import { dismissInstruction } from "@anlg/plugin-windows";

import { InstructionScreen, type InstructionType } from "~/instruction";

export const Route = createFileRoute("/app/instruction")({
  validateSearch: (
    search,
  ): { type: InstructionType; url?: string; integrationId?: string } => ({
    type: ((search as { type?: string }).type ??
      "integration") as InstructionType,
    url: (search as { url?: string }).url,
    integrationId: (search as { integrationId?: string }).integrationId,
  }),
  component: InstructionRoute,
});

function useHandleBack() {
  return useCallback(() => dismissInstruction(), []);
}

function InstructionRoute() {
  const { type, url, integrationId } = Route.useSearch();
  const handleBack = useHandleBack();
  const onBack = useCallback(() => void handleBack(), [handleBack]);
  const onCleanup = useCallback(() => {}, []);

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

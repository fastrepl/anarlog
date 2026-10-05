import { useMountEffect } from "@anlg/ui/hooks/use-mount-effect";

import { useSetSettingValues } from "~/settings/queries";

export function PersistAiSelection({
  type,
  provider,
  model,
}: {
  type: "llm" | "stt";
  provider: string;
  model: string;
}) {
  const setSettingValues = useSetSettingValues();

  useMountEffect(() => {
    setSettingValues(
      type === "llm"
        ? {
            current_llm_provider: provider,
            current_llm_model: model,
          }
        : {
            current_stt_provider: provider,
            current_stt_model: model,
          },
    );
  });

  return null;
}

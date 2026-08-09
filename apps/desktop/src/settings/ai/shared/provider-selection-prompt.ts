import { useLingui } from "@lingui/react/macro";
import { useRef } from "react";

import { sonnerToast } from "@anlg/ui/components/ui/toast";

import { setSettingValues } from "~/settings/queries";

export function useProviderSelectionPrompt({
  providerType,
  providerId,
  providerName,
  currentProvider,
  providerStateReady,
  storedApiKey,
}: {
  providerType: "llm" | "stt";
  providerId: string;
  providerName: string;
  currentProvider?: string;
  providerStateReady: boolean;
  storedApiKey?: string;
}) {
  const { t } = useLingui();
  const hadApiKeyRef = useRef<boolean | null>(null);

  if (providerStateReady && hadApiKeyRef.current === null) {
    hadApiKeyRef.current = !!storedApiKey?.trim();
  }

  return (savedApiKey: string) => {
    const hasApiKey = !!savedApiKey.trim();
    const hadApiKey = hadApiKeyRef.current;

    if (hadApiKey !== null) {
      hadApiKeyRef.current = hasApiKey;
    }

    if (hadApiKey !== false || !hasApiKey || currentProvider === providerId) {
      return;
    }

    sonnerToast.success(t`API key saved`, {
      id: `provider-selection:${providerType}:${providerId}`,
      duration: Infinity,
      description: t`Set ${providerName} as the current provider?`,
      action: {
        label: t`Set as current`,
        onClick: () => {
          void setSettingValues(
            providerType === "llm"
              ? {
                  current_llm_provider: providerId,
                  current_llm_model: "",
                }
              : {
                  current_stt_provider: providerId,
                  current_stt_model: "",
                },
          ).catch((error) => {
            console.error(
              `[settings] failed to select ${providerType} provider`,
              error,
            );
          });
        },
      },
    });
  };
}

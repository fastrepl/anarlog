import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
} from "react";

import { useBillingAccess } from "~/auth/billing";
import { useConfigValues } from "~/shared/config";
import { useToastAction } from "~/store/zustand/toast-action";

type LlmSettingsContextType = {
  accordionValue: string;
  setAccordionValue: (value: string) => void;
  openHyprAccordion: () => void;
  startTrial: () => void;
  shouldHighlight: boolean;
  hyprAccordionRef: React.RefObject<HTMLDivElement | null>;
};

const LlmSettingsContext = createContext<LlmSettingsContextType | null>(null);

export function LlmSettingsProvider({
  children,
}: {
  children: React.ReactNode;
}) {
  const { current_llm_provider, current_llm_model } = useConfigValues([
    "current_llm_provider",
    "current_llm_model",
  ] as const);
  const hasLlmConfigured = !!(current_llm_provider && current_llm_model);

  const [accordionValue, setAccordionValue] = useState<string>(
    hasLlmConfigured ? "" : "hyprnote",
  );
  const [shouldHighlight, setShouldHighlight] = useState(false);
  const { upgradeToPro } = useBillingAccess();
  const hyprAccordionRef = useRef<HTMLDivElement | null>(null);

  const toastActionTarget = useToastAction((state) => state.target);
  const clearToastActionTarget = useToastAction((state) => state.clearTarget);

  useEffect(() => {
    if (toastActionTarget === "llm") {
      setAccordionValue("hyprnote");
      setShouldHighlight(true);

      const timer = setTimeout(() => {
        hyprAccordionRef.current?.scrollIntoView({
          behavior: "smooth",
          block: "center",
        });
      }, 100);

      clearToastActionTarget();
      return () => clearTimeout(timer);
    }
  }, [toastActionTarget, clearToastActionTarget]);

  useEffect(() => {
    if (hasLlmConfigured && shouldHighlight) {
      setShouldHighlight(false);
    }
  }, [hasLlmConfigured, shouldHighlight]);

  const openHyprAccordion = useCallback(() => {
    setAccordionValue("hyprnote");
  }, []);

  const startTrial = useCallback(() => {
    openHyprAccordion();
    upgradeToPro();
  }, [openHyprAccordion, upgradeToPro]);

  return (
    <LlmSettingsContext.Provider
      value={{
        accordionValue,
        setAccordionValue,
        openHyprAccordion,
        startTrial,
        shouldHighlight,
        hyprAccordionRef,
      }}
    >
      {children}
    </LlmSettingsContext.Provider>
  );
}

export function useLlmSettings() {
  const context = useContext(LlmSettingsContext);
  if (!context) {
    throw new Error("useLlmSettings must be used within LlmSettingsProvider");
  }
  return context;
}

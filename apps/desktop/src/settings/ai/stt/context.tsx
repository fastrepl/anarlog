import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
} from "react";

import {
  commands as localSttCommands,
  type LocalModel,
} from "@hypr/plugin-local-stt";

import { useBillingAccess } from "~/auth/billing";
import { useConfigValues } from "~/shared/config";
import { useToastAction } from "~/store/zustand/toast-action";

type SttSettingsContextType = {
  accordionValue: string;
  setAccordionValue: (value: string) => void;
  openHyprAccordion: () => void;
  startDownload: (model: LocalModel) => void;
  startTrial: () => void;
  shouldHighlightDownload: boolean;
  hyprAccordionRef: React.RefObject<HTMLDivElement | null>;
};

const SttSettingsContext = createContext<SttSettingsContextType | null>(null);

export function SttSettingsProvider({
  children,
}: {
  children: React.ReactNode;
}) {
  const { current_stt_provider, current_stt_model } = useConfigValues([
    "current_stt_provider",
    "current_stt_model",
  ] as const);
  const hasSttConfigured = !!(current_stt_provider && current_stt_model);

  const [accordionValue, setAccordionValue] = useState<string>(
    hasSttConfigured ? "" : "hyprnote",
  );
  const [shouldHighlight, setShouldHighlight] = useState(false);
  const { upgradeToPro } = useBillingAccess();
  const hyprAccordionRef = useRef<HTMLDivElement | null>(null);

  const toastActionTarget = useToastAction((state) => state.target);
  const clearToastActionTarget = useToastAction((state) => state.clearTarget);

  useEffect(() => {
    if (toastActionTarget === "stt") {
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
    if (hasSttConfigured && shouldHighlight) {
      setShouldHighlight(false);
    }
  }, [hasSttConfigured, shouldHighlight]);

  const openHyprAccordion = useCallback(() => {
    setAccordionValue("hyprnote");
  }, []);

  const startDownload = useCallback(
    (model: LocalModel) => {
      openHyprAccordion();
      void localSttCommands.downloadModel(model);
    },
    [openHyprAccordion],
  );

  const startTrial = useCallback(() => {
    openHyprAccordion();
    upgradeToPro();
  }, [openHyprAccordion, upgradeToPro]);

  return (
    <SttSettingsContext.Provider
      value={{
        accordionValue,
        setAccordionValue,
        openHyprAccordion,
        startDownload,
        startTrial,
        shouldHighlightDownload: shouldHighlight,
        hyprAccordionRef,
      }}
    >
      {children}
    </SttSettingsContext.Provider>
  );
}

export function useSttSettings() {
  const context = useContext(SttSettingsContext);
  if (!context) {
    throw new Error("useSttSettings must be used within SttSettingsProvider");
  }
  return context;
}

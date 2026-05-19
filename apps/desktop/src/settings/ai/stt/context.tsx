import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
} from "react";

import {
  commands as localSttCommands,
  type LocalModel,
} from "@hypr/plugin-local-stt";

import { useBillingAccess } from "~/auth/billing";
import { useToastAction } from "~/store/zustand/toast-action";

type SttSettingsContextType = {
  accordionValue: string;
  setAccordionValue: (value: string) => void;
  startDownload: (model: LocalModel) => void;
  startTrial: () => void;
};

const SttSettingsContext = createContext<SttSettingsContextType | null>(null);

export function SttSettingsProvider({
  children,
}: {
  children: React.ReactNode;
}) {
  const [accordionValue, setAccordionValue] = useState<string>("");
  const { upgradeToPro } = useBillingAccess();

  const toastActionTarget = useToastAction((state) => state.target);
  const clearToastActionTarget = useToastAction((state) => state.clearTarget);

  useEffect(() => {
    if (toastActionTarget === "stt") {
      clearToastActionTarget();
    }
  }, [toastActionTarget, clearToastActionTarget]);

  const startDownload = useCallback((model: LocalModel) => {
    void localSttCommands.downloadModel(model);
  }, []);

  const startTrial = useCallback(() => {
    upgradeToPro();
  }, [upgradeToPro]);

  return (
    <SttSettingsContext.Provider
      value={{
        accordionValue,
        setAccordionValue,
        startDownload,
        startTrial,
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

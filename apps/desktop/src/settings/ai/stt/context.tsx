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

import { useBillingAccess } from "~/auth/billing-context";
import { useToastAction } from "~/store/zustand/toast-action";

type SttSettingsContextType = {
  accordionValue: string;
  setAccordionValue: (value: string) => void;
  startDownload: (model: LocalModel) => void;
  queuedDownloads: LocalModel[];
  startTrial: () => void;
};

const SttSettingsContext = createContext<SttSettingsContextType | null>(null);

const DOWNLOAD_PROGRESS_GRACE_MS = 10_000;

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

  const [queuedDownloads, setQueuedDownloads] = useState<LocalModel[]>([]);
  const queuedDownloadsRef = useRef<Set<LocalModel>>(new Set());

  const startDownload = useCallback((model: LocalModel) => {
    if (queuedDownloadsRef.current.has(model)) {
      return;
    }

    const dequeue = () => {
      queuedDownloadsRef.current.delete(model);
      setQueuedDownloads([...queuedDownloadsRef.current]);
    };

    queuedDownloadsRef.current.add(model);
    setQueuedDownloads([...queuedDownloadsRef.current]);
    void localSttCommands.downloadModel(model).then(
      // The command resolves when the download starts, not when it finishes.
      // Keep the queue entry until progress events take over the row state,
      // so the gap cannot accept another click.
      () => setTimeout(dequeue, DOWNLOAD_PROGRESS_GRACE_MS),
      dequeue,
    );
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
        queuedDownloads,
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

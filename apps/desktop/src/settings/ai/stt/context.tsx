import { t } from "@lingui/core/macro";
import {
  createContext,
  useCallback,
  useContext,
  useRef,
  useState,
} from "react";

import {
  commands as localSttCommands,
  type LocalModel,
} from "@anlg/plugin-local-stt";
import { toast } from "@anlg/ui/components/ui/toast";

import { useBillingAccess } from "~/auth/billing-context";
import { usePendingSttSelection } from "~/store/zustand/pending-stt-selection";

type SttSettingsContextType = {
  accordionValue: string;
  setAccordionValue: (value: string) => void;
  startDownload: (model: LocalModel, provider: string) => void;
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

  const [queuedDownloads, setQueuedDownloads] = useState<LocalModel[]>([]);
  const queuedDownloadsRef = useRef<Set<LocalModel>>(new Set());

  const startDownload = useCallback((model: LocalModel, provider: string) => {
    if (queuedDownloadsRef.current.has(model)) {
      return;
    }

    const dequeue = () => {
      queuedDownloadsRef.current.delete(model);
      setQueuedDownloads([...queuedDownloadsRef.current]);
    };

    queuedDownloadsRef.current.add(model);
    setQueuedDownloads([...queuedDownloadsRef.current]);
    const selection = { provider, model };
    usePendingSttSelection.setState((state) => ({
      selection,
      queuedDownloads: [
        ...state.queuedDownloads.filter((queued) => queued !== model),
        model,
      ],
    }));
    const clearPendingSelection = () => {
      usePendingSttSelection.setState((state) => ({
        queuedDownloads: state.queuedDownloads.filter(
          (queued) => queued !== model,
        ),
      }));
      if (usePendingSttSelection.getState().selection === selection) {
        usePendingSttSelection.setState({ selection: null });
      }
    };
    void localSttCommands.downloadModel(model).then(
      (result) => {
        if (result.status === "error") {
          toast.error(t`Model download couldn’t start`, {
            description: result.error,
          });
          dequeue();
          clearPendingSelection();
          return;
        }

        // The command resolves when the download starts, not when it finishes.
        // Keep the queue entry until progress events take over the row state,
        // so the gap cannot accept another click.
        setTimeout(dequeue, DOWNLOAD_PROGRESS_GRACE_MS);
      },
      (error) => {
        toast.error(t`Model download couldn’t start`, {
          description: error instanceof Error ? error.message : String(error),
        });
        dequeue();
        clearPendingSelection();
      },
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

import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useRef,
  type ReactNode,
} from "react";

import type { TranscriptWordSelection } from "./selection";

export type TranscriptSelectionSource = () => Array<
  [string, TranscriptWordSelection]
>;

type TranscriptSelectionRegister = (
  sourceId: string,
  getEntries: TranscriptSelectionSource,
) => () => void;

type TranscriptSelectionState = {
  selectMode: boolean;
  isSelected: (key: string) => boolean;
};

const TranscriptSelectionRegisterContext =
  createContext<TranscriptSelectionRegister>(() => () => {});

const TranscriptSelectionStateContext = createContext<TranscriptSelectionState>(
  {
    selectMode: false,
    isSelected: () => false,
  },
);

export function TranscriptSelectionProvider({
  selectMode,
  selectedKeys,
  registerSource,
  children,
}: {
  selectMode: boolean;
  selectedKeys: ReadonlySet<string>;
  registerSource: TranscriptSelectionRegister;
  children: ReactNode;
}) {
  const isSelected = useCallback(
    (key: string) => selectedKeys.has(key),
    [selectedKeys],
  );
  const state = useMemo(
    () => ({ selectMode, isSelected }),
    [isSelected, selectMode],
  );

  return (
    <TranscriptSelectionRegisterContext.Provider value={registerSource}>
      <TranscriptSelectionStateContext.Provider value={state}>
        {children}
      </TranscriptSelectionStateContext.Provider>
    </TranscriptSelectionRegisterContext.Provider>
  );
}

export function useRegisterTranscriptSelectionSource() {
  return useContext(TranscriptSelectionRegisterContext);
}

export function useTranscriptSelectionState() {
  return useContext(TranscriptSelectionStateContext);
}

export function useTranscriptSelectionSources() {
  const sourcesRef = useRef(new Map<string, TranscriptSelectionSource>());
  const registerSource = useCallback<TranscriptSelectionRegister>(
    (sourceId, getEntries) => {
      sourcesRef.current.set(sourceId, getEntries);
      return () => {
        sourcesRef.current.delete(sourceId);
      };
    },
    [],
  );
  const collectEntries = useCallback((sourceIds: string[]) => {
    const entries = new Map<string, TranscriptWordSelection>();
    const order: string[] = [];
    for (const sourceId of sourceIds) {
      const getEntries = sourcesRef.current.get(sourceId);
      if (!getEntries) {
        continue;
      }
      for (const [key, selection] of getEntries()) {
        if (!entries.has(key)) {
          order.push(key);
        }
        entries.set(key, selection);
      }
    }
    return { order, entries };
  }, []);

  return { registerSource, collectEntries };
}

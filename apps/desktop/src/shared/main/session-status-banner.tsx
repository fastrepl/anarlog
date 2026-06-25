import { AnimatePresence, motion } from "motion/react";
import {
  createContext,
  useContext,
  useLayoutEffect,
  useState,
  type Dispatch,
  type SetStateAction,
} from "react";
import { createPortal } from "react-dom";

import { cn } from "@hypr/utils";

import { useMainContentCenterOffset } from "./content-offset";

import { useUndoDelete } from "~/store/zustand/undo-delete";

type SessionStatusBannerState = {
  skipReason: string | null;
} | null;

const SessionStatusBannerStateContext =
  createContext<SessionStatusBannerState>(null);
const SessionStatusBannerSetterContext = createContext<Dispatch<
  SetStateAction<SessionStatusBannerState>
> | null>(null);

export function SessionStatusBannerProvider({
  children,
}: {
  children: React.ReactNode;
}) {
  const [banner, setBanner] = useState<SessionStatusBannerState>(null);

  return (
    <SessionStatusBannerStateContext.Provider value={banner}>
      <SessionStatusBannerSetterContext.Provider value={setBanner}>
        {children}
      </SessionStatusBannerSetterContext.Provider>
    </SessionStatusBannerStateContext.Provider>
  );
}

export function useSessionStatusBanner({
  skipReason,
}: {
  skipReason: string | null;
}) {
  const setBanner = useContext(SessionStatusBannerSetterContext);

  useLayoutEffect(() => {
    if (!setBanner) {
      return;
    }

    setBanner({ skipReason });

    return () => {
      setBanner(null);
    };
  }, [setBanner, skipReason]);
}

export function MainSessionStatusBannerHost() {
  const banner = useContext(SessionStatusBannerStateContext);
  const hasUndoDeleteToast = useUndoDelete(
    (state) => Object.keys(state.pendingDeletions).length > 0,
  );
  const contentOffset = useMainContentCenterOffset();

  if (typeof document === "undefined" || !banner || !banner.skipReason) {
    return null;
  }

  return createPortal(
    <AnimatePresence>
      <motion.div
        key={banner.skipReason}
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        transition={{ duration: 0.3, ease: "easeOut" }}
        style={{ left: `calc(50% + ${contentOffset}px)` }}
        className={cn([
          "fixed z-50 -translate-x-1/2",
          "text-center text-xs whitespace-nowrap",
          "text-red-400",
          hasUndoDeleteToast ? "bottom-1" : "bottom-6",
        ])}
      >
        {banner.skipReason}
      </motion.div>
    </AnimatePresence>,
    document.body,
  );
}

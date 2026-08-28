import * as stylex from "@stylexjs/stylex";
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

import { mergeStyleXProps } from "@anlg/ui/lib/stylex";

import { useMainContentCenterOffset } from "./content-offset";

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
        {...mergeStyleXProps(styles.banner, undefined, {
          left: `calc(50% + ${contentOffset}px)`,
        })}
      >
        {banner.skipReason}
      </motion.div>
    </AnimatePresence>,
    document.body,
  );
}

const styles = stylex.create({
  banner: {
    bottom: "1.5rem",
    color: "rgb(248 113 113)",
    fontSize: "0.75rem",
    position: "fixed",
    textAlign: "center",
    transform: "translateX(-50%)",
    whiteSpace: "nowrap",
    zIndex: 50,
  },
});

export { styles as sessionStatusBannerStyles };

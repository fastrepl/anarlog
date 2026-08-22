import { create } from "zustand";

import { authenticateDevice, isDeviceAuthAvailable } from "./auth";

type AppLockState = {
  available: boolean | null;
  authenticating: boolean;
  appUnlocked: boolean;
  revealedNoteIds: Record<string, true>;
  refreshAvailability: () => Promise<boolean>;
  authenticate: (reason: string) => Promise<boolean>;
  unlockApp: (reason: string) => Promise<boolean>;
  lockApp: () => void;
  revealNote: (sessionId: string, reason: string) => Promise<boolean>;
  markNoteRevealed: (sessionId: string) => void;
  concealNote: (sessionId: string) => void;
  isNoteRevealed: (sessionId: string) => boolean;
};

export const useAppLock = create<AppLockState>((set, get) => {
  let lockEpoch = 0;

  return {
    available: null,
    authenticating: false,
    appUnlocked: false,
    revealedNoteIds: {},
    refreshAvailability: async () => {
      try {
        const available = await isDeviceAuthAvailable();
        set({ available });
        return available;
      } catch {
        set({ available: false });
        return false;
      }
    },
    authenticate: async (reason) => {
      if (get().authenticating) return false;
      set({ authenticating: true });
      try {
        return await authenticateDevice(reason);
      } finally {
        set({ authenticating: false });
      }
    },
    unlockApp: async (reason) => {
      const epoch = lockEpoch;
      const ok = await get().authenticate(reason);
      if (ok && epoch === lockEpoch) {
        set({ appUnlocked: true });
        return true;
      }
      return false;
    },
    lockApp: () => {
      lockEpoch += 1;
      set({ appUnlocked: false, revealedNoteIds: {} });
    },
    revealNote: async (sessionId, reason) => {
      if (get().revealedNoteIds[sessionId]) return true;
      const epoch = lockEpoch;
      const ok = await get().authenticate(reason);
      if (ok && epoch === lockEpoch) {
        get().markNoteRevealed(sessionId);
        return true;
      }
      return false;
    },
    markNoteRevealed: (sessionId) => {
      set((state) => ({
        revealedNoteIds: { ...state.revealedNoteIds, [sessionId]: true },
      }));
    },
    concealNote: (sessionId) => {
      set((state) => {
        if (!state.revealedNoteIds[sessionId]) return state;
        const revealedNoteIds = { ...state.revealedNoteIds };
        delete revealedNoteIds[sessionId];
        return { revealedNoteIds };
      });
    },
    isNoteRevealed: (sessionId) => Boolean(get().revealedNoteIds[sessionId]),
  };
});

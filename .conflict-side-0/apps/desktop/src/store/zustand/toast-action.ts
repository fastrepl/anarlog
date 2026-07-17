import { create } from "zustand";

type ToastActionTarget = "stt" | null;

interface ToastActionState {
  target: ToastActionTarget;
  setTarget: (target: ToastActionTarget) => void;
  clearTarget: () => void;
}

export const useToastAction = create<ToastActionState>((set) => ({
  target: null,
  setTarget: (target) => set({ target }),
  clearTarget: () => set({ target: null }),
}));

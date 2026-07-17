import { useEffect } from "react";

export function useMountEffect(effect: () => void | (() => void)) {
  useEffect(effect, []);
}

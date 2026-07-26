import { useEffect, useRef } from "react";

export function useStrictModeUnmount(fn: () => void) {
  const mountedRef = useRef(true);
  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      setTimeout(() => {
        if (!mountedRef.current) fn();
      }, 50);
    };
  }, [fn]);
}

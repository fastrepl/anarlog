import { useEffect, useReducer } from "react";

import type { UpdaterSnapshot } from "../../shared/updater";
import {
  INITIAL_UPDATER_SNAPSHOT,
  reduceUpdaterSnapshot,
} from "../../shared/updater";

import { hypr } from "~/bridge";

// Single source of truth for the renderer: main process pushes events, we
// fold them into a snapshot. The banner renders off this snapshot, so a
// freshly-mounted banner picks up "ready" immediately even if the download
// finished earlier in this session.
export function useUpdate(): UpdaterSnapshot {
  const [snapshot, dispatch] = useReducer(
    reduceUpdaterSnapshot,
    INITIAL_UPDATER_SNAPSHOT,
  );

  useEffect(() => {
    const unsubscribe = hypr.updater.subscribe((event) => {
      dispatch(event);
    });

    return () => {
      unsubscribe();
    };
  }, []);

  return snapshot;
}

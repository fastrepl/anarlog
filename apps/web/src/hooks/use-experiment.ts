import { useEffect, useState } from "react";

import {
  EXPERIMENTS,
  type ExperimentKey,
  type ExperimentVariant,
  resolveExperimentVariant,
} from "@/lib/experiments";
import { usePostHogOperation } from "@/providers/posthog";

/**
 * Resolves a PostHog experiment variant for the current visitor.
 *
 * Renders the control variant until flags load, so SSR and no-analytics
 * visitors (dev, GPC, private routes) always see the baseline design.
 * Calling `getFeatureFlag` emits `$feature_flag_called`, which PostHog uses to
 * attribute later conversion events to the exposed variant.
 */
export function useExperiment<K extends ExperimentKey>(key: K) {
  const runOrQueue = usePostHogOperation();
  const [variant, setVariant] = useState<ExperimentVariant<K>>(() =>
    resolveExperimentVariant(key, undefined),
  );
  const [ready, setReady] = useState(false);

  useEffect(() => {
    let cancelled = false;
    const { flag } = EXPERIMENTS[key];
    if (import.meta.env.DEV) {
      const override = new URLSearchParams(window.location.search).get(
        `experiment.${flag}`,
      );
      if (override !== null) {
        setVariant(resolveExperimentVariant(key, override));
        setReady(true);
        return;
      }
    }
    let cancelListener: (() => void) | undefined;
    runOrQueue((client) => {
      if (cancelled) return;
      cancelListener = client.onFeatureFlags(() => {
        if (cancelled) return;
        setVariant(resolveExperimentVariant(key, client.getFeatureFlag(flag)));
        setReady(true);
      });
    });
    return () => {
      cancelled = true;
      cancelListener?.();
    };
  }, [key, runOrQueue]);

  return { variant, ready };
}

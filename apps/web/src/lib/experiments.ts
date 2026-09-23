export const EXPERIMENTS = {
  downloadLayout: {
    flag: "web-download-layout",
    variants: ["control", "three-column"],
  },
} as const satisfies Record<
  string,
  { flag: string; variants: readonly [string, ...string[]] }
>;

export type ExperimentKey = keyof typeof EXPERIMENTS;
export type ExperimentVariant<K extends ExperimentKey> =
  (typeof EXPERIMENTS)[K]["variants"][number];

export function resolveExperimentVariant<K extends ExperimentKey>(
  key: K,
  flagValue: unknown,
): ExperimentVariant<K> {
  const { variants } = EXPERIMENTS[key];
  const fallback = variants[0] as ExperimentVariant<K>;
  if (typeof flagValue !== "string") {
    return fallback;
  }
  return (variants as readonly string[]).includes(flagValue)
    ? (flagValue as ExperimentVariant<K>)
    : fallback;
}

export function experimentProperties<K extends ExperimentKey>(
  key: K,
  variant: ExperimentVariant<K>,
) {
  return {
    experiment: EXPERIMENTS[key].flag,
    variant,
  };
}

import { memo, useMemo, type CSSProperties } from "react";

function mulberry32(seed: number): () => number {
  let value = seed;
  return () => {
    value += 0x6d2b79f5;
    let t = value;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

type DancingSticksProps = {
  color?: string;
  amplitude: number;
  height?: number;
  width?: number;
  stickWidth?: number;
  gap?: number;
};

function generatePattern(count: number): number[] {
  if (count <= 1) {
    return [100];
  }

  const pattern: number[] = [];
  const mid = (count - 1) / 2;
  for (let i = 0; i < count; i++) {
    const distance = Math.abs(i - mid) / mid;
    pattern.push(50 + 50 * (1 - distance));
  }
  return pattern;
}

export const DancingSticks = memo(function DancingSticks({
  color = "#e5e5e5",
  amplitude,
  height,
  width,
  stickWidth,
  gap,
}: DancingSticksProps) {
  const resolvedHeight = height ?? 16;
  const resolvedStickWidth = stickWidth ?? 2;
  const resolvedGap = gap ?? 1;
  const resolvedWidth = width ?? 17;
  const stickCount = Math.max(
    1,
    Math.floor(
      (resolvedWidth + resolvedGap) / (resolvedStickWidth + resolvedGap),
    ),
  );
  const isFlat = amplitude === 0;
  const pattern = useMemo(() => generatePattern(stickCount), [stickCount]);

  const amplitudeScale = useMemo(() => {
    const clamped = Math.max(0, Math.min(1, amplitude));
    return 0.2 + 0.8 * clamped;
  }, [amplitude]);

  const containerStyle = useMemo(
    () =>
      ({
        height: resolvedHeight,
        width: resolvedWidth,
        gap: resolvedGap,
        transform: `scaleY(${amplitudeScale})`,
        transformOrigin: "center",
      }) as CSSProperties,
    [amplitudeScale, resolvedGap, resolvedHeight, resolvedWidth],
  );

  const stickParams = useMemo(
    () =>
      pattern.map((baseLength, index) => {
        const maxScaleY = Math.max(0.25, Math.min(1, baseLength / 100));
        const rng = mulberry32((index + 1) * 10007);
        const speed = 4 + rng() * 3;
        const phase = rng() * Math.PI * 2;
        const durationSeconds = (Math.PI * 2) / speed;
        const delaySeconds = -(phase / (Math.PI * 2)) * durationSeconds;
        return { maxScaleY, durationSeconds, delaySeconds };
      }),
    [pattern],
  );

  if (isFlat) {
    return (
      <div
        className="flex items-center justify-center"
        style={{ height: resolvedHeight, width: resolvedWidth }}
      >
        <div
          className="rounded-full"
          style={{
            width: resolvedWidth,
            height: 1,
            backgroundColor: color,
          }}
        />
      </div>
    );
  }

  return (
    <div
      className="flex origin-center items-center justify-center"
      style={containerStyle}
    >
      {stickParams.map(
        ({ maxScaleY, durationSeconds, delaySeconds }, index) => (
          <div
            key={index}
            className="flex origin-center items-center justify-center"
            style={{
              width: resolvedStickWidth,
              height: resolvedHeight,
              transform: `scaleY(${maxScaleY})`,
            }}
          >
            <div
              className="animate-hypr-dancing-stick w-full origin-center rounded-full"
              style={{
                height: resolvedHeight,
                backgroundColor: color,
                animationDuration: `${durationSeconds}s`,
                animationDelay: `${delaySeconds}s`,
              }}
            />
          </div>
        ),
      )}
    </div>
  );
});

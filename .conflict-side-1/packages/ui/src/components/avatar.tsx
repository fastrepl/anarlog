import { useState, type CSSProperties } from "react";

import { useMountEffect } from "@hypr/ui/hooks/use-mount-effect";

export type AvatarRenderStyle = "dithered" | "smooth";

export type AvatarProps = Readonly<{
  seed: string;
  label?: string;
  size?: number;
  colorCount?: number;
  sphereCount?: number;
  dither?: number;
  renderStyle?: AvatarRenderStyle;
  className?: string;
}>;

type AvatarRecipe = Readonly<{
  seed: string;
  colorCount: number;
  sphereCount: number;
  dither: number;
  renderStyle: AvatarRenderStyle;
}>;

type Rgb = readonly [red: number, green: number, blue: number];

type Sphere = Readonly<{
  x: number;
  y: number;
  radius: number;
  color: Rgb;
}>;

const BAYER_4X4 = [
  0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5,
] as const;
const imageCache = new Map<string, string>();
const MAX_CACHE_ENTRIES = 512;
const RASTER_SIZE = 64;

export function Avatar({
  seed,
  label = seed,
  size = 40,
  colorCount = 4,
  sphereCount = 4,
  dither = 0.3,
  renderStyle = "dithered",
  className,
}: AvatarProps) {
  const recipe = { seed, colorCount, sphereCount, dither, renderStyle };
  const cacheKey = recipeKey(recipe);

  return (
    <AvatarImage
      key={cacheKey}
      cacheKey={cacheKey}
      className={className}
      label={label}
      recipe={recipe}
      size={size}
    />
  );
}

function AvatarImage({
  cacheKey,
  className,
  label,
  recipe,
  size,
}: {
  cacheKey: string;
  className?: string;
  label: string;
  recipe: AvatarRecipe;
  size: number;
}) {
  const [src, setSrc] = useState<string | undefined>(() =>
    imageCache.get(cacheKey),
  );
  const dimension = Math.max(1, size);
  const containerStyle = {
    width: dimension,
    height: dimension,
    display: "inline-flex",
    position: "relative",
    flexShrink: 0,
    overflow: "hidden",
    boxSizing: "border-box",
    border: "1px solid rgb(0 0 0 / 0.1)",
    borderRadius: "9999px",
    background: fallbackGradient(recipe.seed),
    boxShadow: "0 1px 2px rgb(0 0 0 / 0.08)",
  } satisfies CSSProperties;
  const initialsStyle = {
    position: "absolute",
    inset: 0,
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    color: "white",
    fontSize: Math.max(7, dimension * 0.38),
    fontWeight: 600,
    lineHeight: 1,
    letterSpacing: "-0.04em",
    mixBlendMode: "overlay",
  } satisfies CSSProperties;

  useMountEffect(() => {
    const cached = imageCache.get(cacheKey);
    if (cached) {
      imageCache.delete(cacheKey);
      imageCache.set(cacheKey, cached);
      setSrc(cached);
      return;
    }
    if (!canRenderAvatarPng()) return;

    const timeout = window.setTimeout(() => {
      setSrc(renderAvatarPng(recipe));
    }, 0);
    return () => window.clearTimeout(timeout);
  });

  return (
    <span aria-hidden="true" className={className} style={containerStyle}>
      {src ? (
        <img
          alt=""
          draggable={false}
          src={src}
          style={{
            position: "absolute",
            inset: 0,
            width: "100%",
            height: "100%",
            objectFit: "cover",
          }}
        />
      ) : null}
      <span aria-hidden="true" style={initialsStyle}>
        {initials(label)}
      </span>
    </span>
  );
}

function renderAvatarPng(recipe: AvatarRecipe): string | undefined {
  const key = recipeKey(recipe);
  const cached = imageCache.get(key);
  if (cached) {
    imageCache.delete(key);
    imageCache.set(key, cached);
    return cached;
  }

  const canvas = document.createElement("canvas");
  canvas.width = RASTER_SIZE;
  canvas.height = RASTER_SIZE;

  try {
    const context = canvas.getContext("2d", { alpha: false });
    if (!context) return undefined;

    const image = context.createImageData(RASTER_SIZE, RASTER_SIZE);
    const random = mulberry32(hashString(recipe.seed));
    const colors = createPalette(random, recipe.colorCount);
    const angle = random() * Math.PI * 2;
    const spheres = createSpheres(random, colors, recipe.sphereCount);
    const quantizationSteps = 6;

    for (let y = 0; y < RASTER_SIZE; y += 1) {
      for (let x = 0; x < RASTER_SIZE; x += 1) {
        const normalizedX = (x + 0.5) / RASTER_SIZE;
        const normalizedY = (y + 0.5) / RASTER_SIZE;
        const directional = clamp(
          0.5 +
            (normalizedX - 0.5) * Math.cos(angle) +
            (normalizedY - 0.5) * Math.sin(angle),
          0,
          1,
        );
        const baseColor = interpolatePalette(colors, directional);
        const color = blendSpheres(
          baseColor,
          normalizedX,
          normalizedY,
          spheres,
        );
        const threshold =
          ((BAYER_4X4[(y % 4) * 4 + (x % 4)] ?? 7.5) / 16 - 0.5) * 255;
        const pixelIndex = (y * RASTER_SIZE + x) * 4;

        for (let channel = 0; channel < 3; channel += 1) {
          const value = color[channel] ?? 0;
          image.data[pixelIndex + channel] =
            recipe.renderStyle === "dithered"
              ? quantize(value + threshold * recipe.dither, quantizationSteps)
              : Math.round(value);
        }
        image.data[pixelIndex + 3] = 255;
      }
    }

    context.putImageData(image, 0, 0);
    const url = canvas.toDataURL("image/png");
    imageCache.set(key, url);
    if (imageCache.size > MAX_CACHE_ENTRIES) {
      const oldestKey = imageCache.keys().next().value;
      if (oldestKey) imageCache.delete(oldestKey);
    }
    return url;
  } catch {
    return undefined;
  }
}

function canRenderAvatarPng(): boolean {
  return (
    typeof document !== "undefined" &&
    typeof CanvasRenderingContext2D !== "undefined"
  );
}

function fallbackGradient(seed: string): string {
  const random = mulberry32(hashString(seed));
  const colors = createPalette(random, 3);
  const angle = Math.round(random() * 360);
  return `linear-gradient(${angle}deg, ${colors.map(rgbString).join(", ")})`;
}

function rgbString([red, green, blue]: Rgb): string {
  return `rgb(${Math.round(red)} ${Math.round(green)} ${Math.round(blue)})`;
}

function createPalette(random: () => number, colorCount: number): Rgb[] {
  const count = clamp(Math.round(colorCount), 2, 5);
  const baseHue = random() * 360;
  const hueSpreads = [32, 52, 138, 208] as const;
  const spread = hueSpreads[Math.floor(random() * hueSpreads.length)] ?? 52;

  return Array.from({ length: count }, (_, index) => {
    const hue = (baseHue + spread * index + (random() - 0.5) * 18) % 360;
    const saturation = 58 + random() * 24;
    const lightness = 48 + random() * 24;
    return hslToRgb(hue, saturation, lightness);
  });
}

function createSpheres(
  random: () => number,
  colors: readonly Rgb[],
  count: number,
): Sphere[] {
  return Array.from({ length: clamp(Math.round(count), 1, 7) }, (_, index) => ({
    x: -0.1 + random() * 1.2,
    y: -0.1 + random() * 1.2,
    radius: 0.24 + random() * 0.42,
    color: colors[(index + 1) % colors.length] ?? [128, 128, 128],
  }));
}

function blendSpheres(
  base: Rgb,
  x: number,
  y: number,
  spheres: readonly Sphere[],
): Rgb {
  let red = base[0];
  let green = base[1];
  let blue = base[2];

  for (const sphere of spheres) {
    const distanceSquared = (x - sphere.x) ** 2 + (y - sphere.y) ** 2;
    const influence =
      Math.exp(-distanceSquared / (2 * sphere.radius ** 2)) * 0.72;
    red += (sphere.color[0] - red) * influence;
    green += (sphere.color[1] - green) * influence;
    blue += (sphere.color[2] - blue) * influence;
  }

  return [red, green, blue];
}

function interpolatePalette(colors: readonly Rgb[], position: number): Rgb {
  const scaled = position * (colors.length - 1);
  const leftIndex = Math.floor(scaled);
  const rightIndex = Math.min(leftIndex + 1, colors.length - 1);
  const amount = scaled - leftIndex;
  const left = colors[leftIndex] ?? [128, 128, 128];
  const right = colors[rightIndex] ?? left;
  return [
    left[0] + (right[0] - left[0]) * amount,
    left[1] + (right[1] - left[1]) * amount,
    left[2] + (right[2] - left[2]) * amount,
  ];
}

function hslToRgb(hue: number, saturation: number, lightness: number): Rgb {
  const s = saturation / 100;
  const l = lightness / 100;
  const chroma = (1 - Math.abs(2 * l - 1)) * s;
  const segment = (((hue % 360) + 360) % 360) / 60;
  const secondary = chroma * (1 - Math.abs((segment % 2) - 1));
  const [red, green, blue]: Rgb =
    segment < 1
      ? [chroma, secondary, 0]
      : segment < 2
        ? [secondary, chroma, 0]
        : segment < 3
          ? [0, chroma, secondary]
          : segment < 4
            ? [0, secondary, chroma]
            : segment < 5
              ? [secondary, 0, chroma]
              : [chroma, 0, secondary];
  const match = l - chroma / 2;
  return [(red + match) * 255, (green + match) * 255, (blue + match) * 255];
}

function quantize(value: number, steps: number): number {
  return clamp(
    Math.round((clamp(value, 0, 255) / 255) * steps) * (255 / steps),
    0,
    255,
  );
}

function hashString(value: string): number {
  let hash = 2166136261;
  for (const character of value.normalize("NFKC")) {
    hash ^= character.codePointAt(0) ?? 0;
    hash = Math.imul(hash, 16777619);
  }
  return hash >>> 0;
}

function mulberry32(seed: number): () => number {
  let state = seed;
  return () => {
    state += 0x6d2b79f5;
    let value = state;
    value = Math.imul(value ^ (value >>> 15), value | 1);
    value ^= value + Math.imul(value ^ (value >>> 7), value | 61);
    return ((value ^ (value >>> 14)) >>> 0) / 4_294_967_296;
  };
}

function recipeKey(recipe: AvatarRecipe): string {
  return [
    recipe.seed,
    recipe.colorCount,
    recipe.sphereCount,
    recipe.dither,
    recipe.renderStyle,
  ].join("|");
}

function initials(value: string): string {
  return value
    .trim()
    .split(/\s+/)
    .map((part) =>
      Array.from(part.normalize("NFKC").replace(/[^\p{L}\p{N}]/gu, "")),
    )
    .filter((part) => part.length > 0)
    .slice(0, 2)
    .map((part) => part[0] ?? "")
    .join("")
    .toUpperCase();
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(Math.max(value, minimum), maximum);
}

import { useState, type CSSProperties } from "react";

import { useMountEffect } from "@anlg/ui/hooks/use-mount-effect";
import {
  AVATAR_RASTER_SIZE,
  avatarFallbackGradient,
  avatarInitials,
  avatarRecipeKey,
  createAvatarPixels,
  type AvatarRecipe,
  type AvatarRenderStyle,
} from "@anlg/ui/lib/avatar";

export type { AvatarRenderStyle } from "@anlg/ui/lib/avatar";

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

const imageCache = new Map<string, string>();
const MAX_CACHE_ENTRIES = 512;

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
  const cacheKey = avatarRecipeKey(recipe);

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
    borderRadius: "0.5rem",
    background: avatarFallbackGradient(recipe.seed),
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
        {avatarInitials(label)}
      </span>
    </span>
  );
}

function renderAvatarPng(recipe: AvatarRecipe): string | undefined {
  const key = avatarRecipeKey(recipe);
  const cached = imageCache.get(key);
  if (cached) {
    imageCache.delete(key);
    imageCache.set(key, cached);
    return cached;
  }

  const canvas = document.createElement("canvas");
  canvas.width = AVATAR_RASTER_SIZE;
  canvas.height = AVATAR_RASTER_SIZE;

  try {
    const context = canvas.getContext("2d", { alpha: false });
    if (!context) return undefined;

    const image = context.createImageData(
      AVATAR_RASTER_SIZE,
      AVATAR_RASTER_SIZE,
    );
    image.data.set(createAvatarPixels(recipe));

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

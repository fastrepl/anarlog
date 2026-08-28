import * as stylex from "@stylexjs/stylex";
import { useState } from "react";

import { radii } from "@anlg/design-system/tokens.stylex";
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
import { mergeStyleXProps, type StyleXProps } from "@anlg/ui/lib/stylex";

export type { AvatarRenderStyle } from "@anlg/ui/lib/avatar";

export type AvatarProps = Readonly<
  {
    seed: string;
    label?: string;
    size?: number;
    colorCount?: number;
    sphereCount?: number;
    dither?: number;
    renderStyle?: AvatarRenderStyle;
    className?: string;
  } & StyleXProps
>;

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
  sx,
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
      sx={sx}
    />
  );
}

function AvatarImage({
  cacheKey,
  className,
  label,
  recipe,
  size,
  sx,
}: {
  cacheKey: string;
  className?: string;
  label: string;
  recipe: AvatarRecipe;
  size: number;
} & StyleXProps) {
  const [src, setSrc] = useState<string | undefined>(() =>
    imageCache.get(cacheKey),
  );
  const dimension = Math.max(1, size);

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
    <span
      aria-hidden="true"
      {...mergeStyleXProps(
        [
          styles.container,
          styles.containerDynamic(
            dimension,
            avatarFallbackGradient(recipe.seed),
          ),
          sx,
        ],
        className,
      )}
    >
      {src ? (
        <img
          {...stylex.props(styles.image)}
          alt=""
          draggable={false}
          src={src}
        />
      ) : null}
      <span
        aria-hidden="true"
        {...stylex.props([
          styles.initials,
          styles.initialsDynamic(Math.max(7, dimension * 0.38)),
        ])}
      >
        {avatarInitials(label)}
      </span>
    </span>
  );
}

const styles = stylex.create({
  container: {
    borderColor: "rgb(0 0 0 / 0.1)",
    borderRadius: radii.full,
    borderStyle: "solid",
    borderWidth: "1px",
    boxShadow: "0 1px 2px rgb(0 0 0 / 0.08)",
    boxSizing: "border-box",
    display: "inline-flex",
    flexShrink: 0,
    overflow: "hidden",
    position: "relative",
  },
  containerDynamic: (dimension: number, backgroundImage: string) => ({
    backgroundImage,
    height: dimension,
    width: dimension,
  }),
  image: {
    height: "100%",
    inset: 0,
    objectFit: "cover",
    position: "absolute",
    width: "100%",
  },
  initials: {
    alignItems: "center",
    color: "white",
    display: "flex",
    fontWeight: 600,
    inset: 0,
    justifyContent: "center",
    letterSpacing: "-0.04em",
    lineHeight: 1,
    mixBlendMode: "overlay",
    position: "absolute",
  },
  initialsDynamic: (fontSize: number) => ({
    fontSize,
  }),
});

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

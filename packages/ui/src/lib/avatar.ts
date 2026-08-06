export type AvatarRenderStyle = "dithered" | "smooth";

export type AvatarRecipe = Readonly<{
  seed: string;
  colorCount: number;
  sphereCount: number;
  dither: number;
  renderStyle: AvatarRenderStyle;
}>;

export type AvatarRgb = readonly [red: number, green: number, blue: number];

type Sphere = Readonly<{
  x: number;
  y: number;
  radius: number;
  color: AvatarRgb;
}>;

const BAYER_4X4 = [
  0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5,
] as const;

export const AVATAR_RASTER_SIZE = 64;

export function createAvatarPixels(
  recipe: AvatarRecipe,
  size = AVATAR_RASTER_SIZE,
) {
  const dimension = Math.max(1, Math.round(size));
  const pixels = new Uint8ClampedArray(dimension * dimension * 4);
  const random = mulberry32(hashString(recipe.seed));
  const colors = createPalette(random, recipe.colorCount);
  const angle = random() * Math.PI * 2;
  const spheres = createSpheres(random, colors, recipe.sphereCount);
  const quantizationSteps = 6;

  for (let y = 0; y < dimension; y += 1) {
    for (let x = 0; x < dimension; x += 1) {
      const normalizedX = (x + 0.5) / dimension;
      const normalizedY = (y + 0.5) / dimension;
      const directional = clamp(
        0.5 +
          (normalizedX - 0.5) * Math.cos(angle) +
          (normalizedY - 0.5) * Math.sin(angle),
        0,
        1,
      );
      const baseColor = interpolatePalette(colors, directional);
      const color = blendSpheres(baseColor, normalizedX, normalizedY, spheres);
      const threshold =
        ((BAYER_4X4[(y % 4) * 4 + (x % 4)] ?? 7.5) / 16 - 0.5) * 255;
      const pixelIndex = (y * dimension + x) * 4;

      for (let channel = 0; channel < 3; channel += 1) {
        const value = color[channel] ?? 0;
        pixels[pixelIndex + channel] =
          recipe.renderStyle === "dithered"
            ? quantize(value + threshold * recipe.dither, quantizationSteps)
            : Math.round(value);
      }
      pixels[pixelIndex + 3] = 255;
    }
  }

  return pixels;
}

export function createAvatarGradient(seed: string) {
  const random = mulberry32(hashString(seed));
  const colors = createPalette(random, 3);
  const angle = Math.round(random() * 360);
  return { angle, colors } as const;
}

export function avatarFallbackGradient(seed: string) {
  const { angle, colors } = createAvatarGradient(seed);
  return `linear-gradient(${angle}deg, ${colors.map(rgbString).join(", ")})`;
}

export function avatarInitials(value: string) {
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

export function avatarRecipeKey(recipe: AvatarRecipe) {
  return [
    recipe.seed,
    recipe.colorCount,
    recipe.sphereCount,
    recipe.dither,
    recipe.renderStyle,
  ].join("|");
}

function rgbString([red, green, blue]: AvatarRgb) {
  return `rgb(${Math.round(red)} ${Math.round(green)} ${Math.round(blue)})`;
}

function createPalette(random: () => number, colorCount: number): AvatarRgb[] {
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
  colors: readonly AvatarRgb[],
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
  base: AvatarRgb,
  x: number,
  y: number,
  spheres: readonly Sphere[],
): AvatarRgb {
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

function interpolatePalette(
  colors: readonly AvatarRgb[],
  position: number,
): AvatarRgb {
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

function hslToRgb(
  hue: number,
  saturation: number,
  lightness: number,
): AvatarRgb {
  const s = saturation / 100;
  const l = lightness / 100;
  const chroma = (1 - Math.abs(2 * l - 1)) * s;
  const segment = (((hue % 360) + 360) % 360) / 60;
  const secondary = chroma * (1 - Math.abs((segment % 2) - 1));
  const [red, green, blue]: AvatarRgb =
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

function quantize(value: number, steps: number) {
  return clamp(
    Math.round((clamp(value, 0, 255) / 255) * steps) * (255 / steps),
    0,
    255,
  );
}

function hashString(value: string) {
  let hash = 2166136261;
  for (const character of value.normalize("NFKC")) {
    hash ^= character.codePointAt(0) ?? 0;
    hash = Math.imul(hash, 16777619);
  }
  return hash >>> 0;
}

function mulberry32(seed: number) {
  let state = seed;
  return () => {
    state += 0x6d2b79f5;
    let value = state;
    value = Math.imul(value ^ (value >>> 15), value | 1);
    value ^= value + Math.imul(value ^ (value >>> 7), value | 61);
    return ((value ^ (value >>> 14)) >>> 0) / 4_294_967_296;
  };
}

function clamp(value: number, minimum: number, maximum: number) {
  return Math.min(Math.max(value, minimum), maximum);
}

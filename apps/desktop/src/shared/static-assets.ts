import type { SyntheticEvent } from "react";

import manifest from "./static-assets.manifest.json";

import { env } from "~/env";

export function staticAssetUrl(localPath: string) {
  const object = manifest[localPath as keyof typeof manifest];
  const origin = env.VITE_STATIC_ASSETS_URL;
  if (!origin || !object) return localPath;
  return `${origin.replace(/\/$/, "")}/${object.split("/").map(encodeURIComponent).join("/")}`;
}

export function fallbackToLocalAsset(localPath: string) {
  return (event: SyntheticEvent<HTMLImageElement>) => {
    const image = event.currentTarget;
    if (image.getAttribute("src") !== localPath) image.src = localPath;
  };
}

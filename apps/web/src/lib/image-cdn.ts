/**
 * Netlify Image CDN wrapper.
 *
 * Assets in Supabase storage are stored at their original upload size — some
 * team avatars are multi-megabyte PNGs rendered at 30px. Routing them through
 * the Image CDN resizes and re-encodes at the edge so the browser downloads
 * something close to the rendered size.
 */
export function getResizedImageUrl(
  src: string,
  { width, height }: { width: number; height?: number },
) {
  if (!src.startsWith("/") || src.startsWith("/.netlify/")) {
    return src;
  }

  const params = new URLSearchParams({
    url: src,
    w: String(width),
    fm: "webp",
  });
  if (height) {
    params.set("h", String(height));
    params.set("fit", "cover");
  }

  return `/.netlify/images?${params.toString()}`;
}

/**
 * Retina-ready `srcset` for a fixed-size image.
 */
export function getResizedImageSrcSet(src: string, size: number) {
  if (!src.startsWith("/") || src.startsWith("/.netlify/")) {
    return undefined;
  }

  return [1, 2]
    .map(
      (dpr) =>
        `${getResizedImageUrl(src, { width: size * dpr, height: size * dpr })} ${dpr}x`,
    )
    .join(", ");
}

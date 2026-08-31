export const WORKSPACE_LOGO_RASTER_SIZE = 128;
export const MAX_WORKSPACE_LOGO_DATA_LENGTH = 120_000;

const WORKSPACE_LOGO_DATA_PATTERN =
  /^data:image\/jpeg;base64,[A-Za-z0-9+/]+={0,2}$/;

export function isWorkspaceLogoDataUrl(value: string): boolean {
  return (
    value.length >= 30 &&
    value.length <= MAX_WORKSPACE_LOGO_DATA_LENGTH &&
    WORKSPACE_LOGO_DATA_PATTERN.test(value)
  );
}

export async function compressWorkspaceLogo(file: File): Promise<string> {
  const url = URL.createObjectURL(file);
  try {
    const image = await loadImage(url);
    const side = Math.min(image.naturalWidth, image.naturalHeight);
    if (side === 0) throw new Error("image has no pixels");

    const canvas = document.createElement("canvas");
    canvas.width = WORKSPACE_LOGO_RASTER_SIZE;
    canvas.height = WORKSPACE_LOGO_RASTER_SIZE;
    const context = canvas.getContext("2d");
    if (!context) throw new Error("canvas 2d context unavailable");

    // JPEG has no alpha channel; flatten transparency onto white.
    context.fillStyle = "#ffffff";
    context.fillRect(
      0,
      0,
      WORKSPACE_LOGO_RASTER_SIZE,
      WORKSPACE_LOGO_RASTER_SIZE,
    );
    context.imageSmoothingQuality = "high";
    context.drawImage(
      image,
      (image.naturalWidth - side) / 2,
      (image.naturalHeight - side) / 2,
      side,
      side,
      0,
      0,
      WORKSPACE_LOGO_RASTER_SIZE,
      WORKSPACE_LOGO_RASTER_SIZE,
    );
    const dataUrl = canvas.toDataURL("image/jpeg", 0.85);
    if (!isWorkspaceLogoDataUrl(dataUrl)) {
      throw new Error("compressed logo is not a JPEG");
    }
    return dataUrl;
  } finally {
    URL.revokeObjectURL(url);
  }
}

function loadImage(url: string): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve(image);
    image.onerror = () => reject(new Error("failed to load image"));
    image.src = url;
  });
}

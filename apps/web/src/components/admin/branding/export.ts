import { toPng } from "html-to-image";

export async function exportBanner(
  node: HTMLDivElement | null,
  filename = "banner.png",
) {
  if (!node) return;
  node.dataset.exporting = "true";
  try {
    const dataUrl = await toPng(node, { pixelRatio: 2 });
    const link = document.createElement("a");
    link.download = filename;
    link.href = dataUrl;
    link.click();
  } finally {
    delete node.dataset.exporting;
  }
}

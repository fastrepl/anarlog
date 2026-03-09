import { parseYouTubeUrl } from "@hypr/tiptap/shared";

export function Clip({ src }: { src: string }) {
  const parsed = parseYouTubeUrl(src);
  const embedSrc = parsed?.embedUrl ?? src;

  return (
    <div className="not-prose my-8">
      <div
        className="relative w-full overflow-hidden rounded-xs border border-neutral-200"
        style={{ paddingBottom: "56.25%" }}
      >
        <iframe
          src={embedSrc}
          className="absolute inset-0 h-full w-full"
          allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture; web-share"
          referrerPolicy="strict-origin-when-cross-origin"
          allowFullScreen
        />
      </div>
    </div>
  );
}

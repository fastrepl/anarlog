import { createFileRoute, redirect, useNavigate } from "@tanstack/react-router";
import { FileText } from "lucide-react";
import { lazy, Suspense, useState } from "react";

import type { JSONContent } from "@hypr/tiptap/editor";
import { EMPTY_TIPTAP_DOC } from "@hypr/tiptap/shared";
import "@hypr/tiptap/styles.css";
import { cn } from "@hypr/utils";

import { Notepad } from "@/components/notepad";
import { EMPTY_MENTION_CONFIG } from "@/components/transcription/constants";
import { fetchUser } from "@/functions/auth";

const NoteEditor = lazy(() => import("@hypr/tiptap/editor"));

export const Route = createFileRoute("/_view/file-transcription")({
  component: Component,
  validateSearch: (search: Record<string, unknown>) => ({
    id: (search.id as string) || undefined,
  }),
  beforeLoad: async ({ search }) => {
    const user = await fetchUser();
    if (user) {
      throw redirect({
        to: "/app/file-transcription/",
        search: { ...search, tab: undefined },
      });
    }
  },
  head: () => ({
    meta: [
      { title: "Free Audio Transcription Tool - Char" },
      {
        name: "description",
        content:
          "Transcribe audio files to text with AI-powered accuracy. Upload MP3, WAV, M4A, or other audio formats and get instant transcripts powered by Deepgram. Free to use.",
      },
      {
        property: "og:title",
        content: "Free Audio Transcription Tool - Char",
      },
      {
        property: "og:description",
        content:
          "Convert audio to text instantly. Upload your recordings and get accurate AI transcriptions. Supports multiple audio formats including MP3, WAV, and M4A.",
      },
      { property: "og:type", content: "website" },
      {
        property: "og:url",
        content: "https://char.dev/file-transcription",
      },
    ],
  }),
});

function Component() {
  const navigate = useNavigate({ from: Route.fullPath });
  const [noteContent, setNoteContent] = useState<JSONContent>(EMPTY_TIPTAP_DOC);

  const handleUploadClick = () => {
    navigate({
      to: "/auth/",
      search: { flow: "web", redirect: "/file-transcription/" },
    });
  };

  return (
    <Notepad
      title="Char — Web demo"
      store={false}
      tabs={[
        {
          value: "default",
          icon: FileText,
          content: (
            <>
              <div className="px-6 pt-6 pb-24">
                <h2 className="text-muted-foreground text-xl font-semibold">
                  Untitled
                </h2>

                <div className="mt-4">
                  <Suspense fallback={null}>
                    <NoteEditor
                      initialContent={noteContent}
                      handleChange={setNoteContent}
                      mentionConfig={EMPTY_MENTION_CONFIG}
                    />
                  </Suspense>
                </div>
              </div>

              <div className="absolute bottom-6 left-1/2 z-40 -translate-x-1/2">
                <button
                  onClick={handleUploadClick}
                  className={cn([
                    "flex items-center gap-2 px-5 py-2.5",
                    "rounded-full bg-neutral-900",
                    "shadow-lg",
                    "hover:bg-neutral-800",
                    "active:scale-[98%]",
                    "transition-all",
                  ])}
                >
                  <span className="flex h-2.5 w-2.5 rounded-full bg-red-400" />
                  <span className="text-sm font-medium text-white">
                    Upload file
                  </span>
                </button>
              </div>
            </>
          ),
        },
      ]}
    />
  );
}

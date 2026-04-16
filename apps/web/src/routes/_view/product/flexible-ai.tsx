import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";
import type { ReactNode } from "react";

import { cn } from "@hypr/utils";

import { CTASection } from "@/components/cta-section";
import { DownloadButton } from "@/components/download-button";
import { GithubStars } from "@/components/github-stars";
import { FAQ, FAQItem } from "@/components/mdx-shared";

export const Route = createFileRoute("/_view/product/flexible-ai")({
  component: Component,
  head: () => ({
    meta: [
      { title: "Flexible AI - Char" },
      {
        name: "description",
        content:
          "The only AI note-taker that lets you choose your preferred STT and LLM provider. Cloud, BYOK, or fully local.",
      },
      { name: "robots", content: "noindex, nofollow" },
    ],
  }),
});

const setupOptions = [
  {
    icon: "mdi:cloud-outline",
    eyebrow: "Managed",
    title: "Char Cloud",
    detail: "$8/month",
    description:
      "Start with a setup that works immediately. No API keys, no provider decisions, no configuration drag.",
  },
  {
    icon: "mdi:key-outline",
    eyebrow: "Bring your own stack",
    title: "BYOK",
    detail: "Free",
    description:
      "Use your existing OpenAI, Anthropic, Deepgram, or other provider credits directly without markup.",
  },
  {
    icon: "mdi:laptop-account",
    eyebrow: "Private by default",
    title: "Fully local",
    detail: "On-device",
    description:
      "Run transcription and summaries on your machine when sensitive conversations should never leave it.",
  },
];

const switchBenefits = [
  {
    title: "Start simple, change later",
    description:
      "Begin with Char Cloud, then move to BYOK or local once you know your workflow and constraints.",
  },
  {
    title: "Match the meeting, not the plan",
    description:
      "Use local AI for sensitive calls, cloud models for tougher reasoning, or BYOK when you want cost control.",
  },
  {
    title: "Re-run older notes with better models",
    description:
      "When a stronger model becomes available, process existing transcripts again instead of starting over.",
  },
  {
    title: "Your notes stay put",
    description:
      "The AI layer is flexible, but the notes remain Markdown files on your device either way.",
  },
];

const localCapabilities = [
  {
    icon: "mdi:microphone-outline",
    title: "Local transcription with Whisper",
    description:
      "Download Whisper through Ollama or LM Studio and transcribe meetings without any API calls.",
  },
  {
    icon: "mdi:brain",
    title: "Local summaries and chat",
    description:
      "Run Llama, Mistral, Qwen, or other open models locally for summaries, action items, and question answering.",
  },
];
function Component() {
  return <div />;
}

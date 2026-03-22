import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";
import { useState } from "react";

import { cn } from "@hypr/utils";

import { Image } from "@/components/image";
import { useAnalytics } from "@/hooks/use-posthog";

export const Route = createFileRoute("/_view/download/")({
  component: Component,
});

function Component() {
  return (
    <div className="min-h-screen">
      <div className="mx-auto">
        <div
          className={cn([
            "flex items-center justify-center gap-2 text-left",
            "border-border-subtle bg-surface-subtle border-b",
            "px-4 py-3",
            "text-color font-mono text-sm",
            "hover:bg-surface-subtle/80 transition-all",
          ])}
        >
          <span>
            Mac (Apple Silicon) features on-device speech-to-text. Intel Mac
            available with cloud-based transcription.
          </span>
        </div>

        <div className="py-12">
          <section className="px-4 py-16">
            <div className="mx-auto mb-16 flex max-w-2xl flex-col gap-6 text-left">
              <h1 className="text-color font-mono text-4xl tracking-tight sm:text-5xl">
                Download Char
              </h1>
              <p className="text-fg-muted text-lg sm:text-xl">
                Choose your platform to get started with Char
              </p>
            </div>

            <div className="mb-16">
              <h2 className="text-color mb-6 text-left font-mono text-2xl tracking-tight">
                macOS
              </h2>
              <div className="mx-auto grid max-w-2xl grid-cols-1 gap-6 md:grid-cols-2">
                <DownloadCard
                  iconName="simple-icons:apple"
                  spec="macOS 14.2+ (Apple Silicon)"
                  downloadUrl="/download/apple-silicon"
                  nightlyDownloadUrl="/download/nightly/apple-silicon"
                  platform="macos-apple-silicon"
                />
                <DownloadCard
                  iconName="simple-icons:apple"
                  spec="macOS 14.2+ (Intel)"
                  downloadUrl="/download/apple-intel"
                  nightlyDownloadUrl="/download/nightly/apple-intel"
                  platform="macos-intel"
                />
              </div>
            </div>
          </section>
          <FAQSection />
          <CTASection />
        </div>
      </div>
    </div>
  );
}

function DownloadCard({
  iconName,
  spec,
  downloadUrl,
  nightlyDownloadUrl,
  platform,
}: {
  iconName: string;
  spec: string;
  downloadUrl: string;
  nightlyDownloadUrl: string;
  platform: string;
}) {
  const { track } = useAnalytics();
  const [isNightly, setIsNightly] = useState(false);

  const handleClick = () => {
    track("download_clicked", {
      platform: isNightly ? `${platform}-nightly` : platform,
      spec,
      source: "download_page",
    });
  };

  return (
    <div
      className={cn([
        "flex flex-col items-center rounded-xs border p-6 transition-all duration-200",
        isNightly
          ? ["border-blue-200 bg-blue-50/50"]
          : ["border-neutral-100 bg-white hover:bg-stone-50"],
      ])}
    >
      <Icon icon={iconName} className="text-color mb-4 text-5xl" />
      <p className="text-fg-muted mb-6 text-left text-sm">{spec}</p>

      <div className="group/tooltip relative w-full">
        <a
          href={isNightly ? nightlyDownloadUrl : downloadUrl}
          download
          onClick={handleClick}
          className={cn([
            "group flex h-11 w-full items-center justify-center gap-2 rounded-full px-4 text-base font-medium shadow-md transition-all hover:scale-[102%] hover:shadow-lg active:scale-[98%]",
            isNightly
              ? ["bg-linear-to-b from-[#03BCF1] to-[#127FE5] text-white"]
              : ["bg-linear-to-t from-stone-600 to-stone-500 text-white"],
          ])}
        >
          {isNightly ? "Download Nightly" : "Download"}
          <Icon
            icon="ph:arrow-circle-right"
            className="text-xl transition-transform group-hover:translate-x-1"
          />
        </a>
      </div>

      <button
        onClick={() => setIsNightly(!isNightly)}
        className="mt-3 cursor-pointer text-xs text-neutral-400 transition-colors hover:text-neutral-600"
      >
        {isNightly
          ? "It might be less stable though."
          : "Want to get faster updates?"}
      </button>
    </div>
  );
}

function FAQSection() {
  const faqs = [
    {
      question: "Which platforms are currently supported?",
      answer:
        "macOS 14.2+ with both Apple Silicon and Intel is currently available. Windows is planned for March 2026, and iOS/Android for April 2026.",
    },
    {
      question: "What's special about the Mac version?",
      answer:
        "The Mac (Apple Silicon) version features on-device speech-to-text, ensuring your audio never leaves your device for complete privacy.",
    },
    {
      question: "Do I need an internet connection?",
      answer:
        "For the free version with local transcription on Mac, no internet is required. Cloud features in the Pro plan require an internet connection.",
    },
    {
      question: "How do I get started after downloading?",
      answer:
        "Simply install the app and launch it. For the free version, you can optionally bring your own API keys for LLM features. Check our documentation for detailed setup instructions.",
    },
  ];

  return (
    <section className="laptop:px-0 px-4 py-16">
      <div className="mx-auto max-w-3xl">
        <h2 className="text-color mb-16 text-left font-mono text-3xl">
          Frequently Asked Questions
        </h2>
        <div className="flex flex-col gap-6">
          {faqs.map((faq, idx) => (
            <div
              key={idx}
              className="border-border-subtle border-b pb-6 last:border-b-0"
            >
              <h3 className="text-color mb-2 text-lg font-medium">
                {faq.question}
              </h3>
              <p className="text-fg-muted">{faq.answer}</p>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

function CTASection() {
  return (
    <section className="laptop:px-0 bg-linear-to-t from-stone-50/30 to-stone-100/30 px-4 py-16">
      <div className="flex flex-col items-center gap-6 text-left">
        <div className="mb-4 flex size-40 items-center justify-center rounded-[48px] border border-neutral-100 bg-transparent shadow-2xl">
          <Image
            src="/api/images/hyprnote/icon.png"
            alt="Char"
            width={144}
            height={144}
            className="mx-auto size-36 rounded-[40px] border border-neutral-100"
          />
        </div>
        <h2 className="text-color font-mono text-2xl sm:text-3xl">
          Need something else?
        </h2>
        <p className="text-fg-muted mx-auto max-w-2xl text-lg">
          Book a call to discuss custom solutions for your specific needs
        </p>
        <div className="pt-6">
          <Link
            to="/founders/"
            search={{ source: "download" }}
            className="flex h-12 items-center justify-center rounded-full bg-linear-to-t from-stone-600 to-stone-500 px-6 text-base text-white shadow-md transition-all hover:scale-[102%] hover:shadow-lg active:scale-[98%] sm:text-lg"
          >
            Book a call
          </Link>
        </div>
      </div>
    </section>
  );
}

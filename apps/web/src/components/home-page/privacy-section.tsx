import {
  Cloud,
  Cpu,
  type Icon as PhosphorIcon,
  Key,
} from "@phosphor-icons/react";

import { cn } from "@anlg/utils";

const privacyCommitments = [
  {
    title: "Invisible while you meet",
    description:
      "No bot joins the meeting, and Anarlog stays hidden while sharing screens.",
    visual: "meeting",
  },
  {
    title: "Local by default",
    description:
      "Your notes, transcripts, attachments, and recordings stay on your device by default.",
    visual: "files",
  },
  {
    title: "Own your AI stack",
    description:
      "Run models on your device, bring your own keys, or use our AI.",
    visual: "key",
  },
];

export function PrivacySection() {
  return (
    <section className="py-16 md:py-20">
      <div>
        <h2 className="font-hand text-3xl leading-none font-semibold text-[#756b5d]">
          Private from call to file
        </h2>
        <p className="mx-auto mt-6 max-w-2xl text-lg leading-8 text-[#4f4940]">
          Anarlog works quietly from your desktop, without joining your calls,
          forcing your data into the cloud, or locking it inside our app.
        </p>
      </div>

      <div className="relative left-1/2 mt-6 w-screen max-w-[1120px] -translate-x-1/2">
        <div className="grid gap-4 md:flex md:items-start md:justify-between md:gap-0">
          {privacyCommitments.map((commitment) => {
            return (
              <div
                key={commitment.description}
                className="flex flex-col px-6 py-3 text-center md:w-[31%] md:p-4"
              >
                <PrivacyVisual type={commitment.visual} />
                <h3 className="mt-5 text-base font-medium text-[#4f4940] md:mt-7">
                  {commitment.title}
                </h3>
                <p className="mx-auto mt-1 max-w-[17rem] text-sm leading-6 text-[#4f4940]">
                  {commitment.description}
                </p>
              </div>
            );
          })}
        </div>
      </div>
    </section>
  );
}

function PrivacyVisual({
  type,
}: {
  type: (typeof privacyCommitments)[number]["visual"];
}) {
  if (type === "files") {
    return (
      <div className="flex h-20 items-center justify-center gap-2 select-none md:h-28 md:w-full md:justify-between md:gap-1">
        <img
          src="/icons/file.webp"
          alt=""
          className="w-10 rotate-[3deg] object-contain transition-transform duration-300 ease-out hover:rotate-[7deg]"
          draggable={false}
        />
        <img
          src="/icons/file.webp"
          alt=""
          className="w-10 rotate-[-5deg] object-contain transition-transform duration-300 ease-out hover:rotate-[-9deg]"
          draggable={false}
        />
        <img
          src="/icons/folderchar.svg"
          alt=""
          className="w-14 object-contain transition-transform duration-300 ease-out hover:rotate-[3deg]"
          draggable={false}
        />
        <img
          src="/icons/file.webp"
          alt=""
          className="w-10 rotate-[6deg] object-contain transition-transform duration-300 ease-out hover:rotate-[10deg]"
          draggable={false}
        />
        <img
          src="/icons/file.webp"
          alt=""
          className="w-10 rotate-[-4deg] object-contain transition-transform duration-300 ease-out hover:rotate-[-8deg]"
          draggable={false}
        />
      </div>
    );
  }

  if (type === "key") {
    return (
      <div className="flex h-24 items-center justify-center select-none md:h-28 md:w-full">
        <div
          className="relative h-20 w-44 md:h-24 md:w-48"
          role="img"
          aria-label="AI option cards cycling between cloud, key, and chip"
        >
          <AiOptionPlayingCard
            className="ai-option-card-cloud ai-option-card-red"
            rank="C"
            IconComponent={Cloud}
          />
          <AiOptionPlayingCard
            className="ai-option-card-key"
            rank="K"
            IconComponent={Key}
          />
          <AiOptionPlayingCard
            className="ai-option-card-chip"
            rank="O"
            IconComponent={Cpu}
          />
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-20 items-center justify-center select-none md:h-28 md:w-full">
      <div className="flex w-full max-w-[260px] items-center gap-3 rounded-2xl border border-neutral-200 bg-white py-2 pr-3 pl-4 text-left shadow-[0_3px_10px_rgba(24,22,19,0.04)]">
        <img
          src="/icons/google-meet.svg"
          alt=""
          className="h-7 w-7 object-contain"
          draggable={false}
        />
        <div className="flex flex-col gap-1">
          <span className="text-sm font-medium text-stone-800">
            Sprint 3 planning
          </span>
          <span className="text-sm text-stone-400">5 participants</span>
        </div>
        <div
          className="meeting-audio-bars ml-auto flex h-6 items-center gap-0.5"
          aria-hidden="true"
        >
          <span className="meeting-audio-bar" />
          <span className="meeting-audio-bar" />
          <span className="meeting-audio-bar" />
        </div>
      </div>
    </div>
  );
}

function AiOptionPlayingCard({
  className,
  rank,
  IconComponent,
}: {
  className: string;
  rank: string;
  IconComponent: PhosphorIcon;
}) {
  return (
    <div className={cn(["ai-option-card", className])}>
      <span className="ai-option-card-corner ai-option-card-corner-top">
        <span className="ai-option-card-rank">{rank}</span>
      </span>
      <div className="ai-option-card-face">
        <IconComponent aria-hidden="true" />
      </div>
      <span className="ai-option-card-corner ai-option-card-corner-bottom">
        <span className="ai-option-card-rank">{rank}</span>
      </span>
    </div>
  );
}

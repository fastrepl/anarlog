import "./dictation-section.css";

import { Link } from "@tanstack/react-router";
import { useReducedMotion } from "motion/react";
import { useId, useRef, useState } from "react";

import { useMountEffect } from "@/hooks/useMountEffect";

const rawSpeechParts = [
  { text: "um ", filler: true },
  { text: "thanks for making time today. ", filler: false },
  { text: "you know ", filler: true },
  {
    text: "the standup notes are in Anarlog, and the client liked the direction. ",
    filler: false,
  },
  { text: "like ", filler: true },
  {
    text: "we should make the next version easier to get started with. ",
    filler: false,
  },
  { text: "uh ", filler: true },
  {
    text: "let’s simplify the first screen and put the main action at the top. ",
    filler: false,
  },
  { text: "I mean ", filler: true },
  {
    text: "people should know what to do without reading a whole guide. ",
    filler: false,
  },
  { text: "so, um ", filler: true },
  { text: "I’ll send the updated proposal tomorrow morning. ", filler: false },
  { text: "you know ", filler: true },
  {
    text: "please add your feedback before Thursday so we can review it together. ",
    filler: false,
  },
  { text: "and, like ", filler: true },
  {
    text: "if anything is unclear, leave a comment next to the relevant section. ",
    filler: false,
  },
  { text: "um ", filler: true },
  {
    text: "after that, we can share the final plan with the team and get started. ",
    filler: false,
  },
];

const polishedSpeech =
  "Thanks for making time today. The standup notes are in Anarlog, and the client liked the direction. We should make the next version easier to get started with. Let’s simplify the first screen and put the main action at the top. People should know what to do without reading a whole guide. I’ll send the updated proposal tomorrow morning. Please add your feedback before Thursday so we can review it together. If anything is unclear, leave a comment next to the relevant section. After that, we can share the final plan with the team and get started. ";

const rawSpeech = rawSpeechParts.map((part) => part.text).join("");
const streamCopies = 3;
const rawSpeechStreamParts = Array.from(
  { length: streamCopies },
  (_, repeatIndex) =>
    rawSpeechParts.map((part, partIndex) => ({
      ...part,
      id: `${repeatIndex}-${partIndex}`,
    })),
).flat();
const polishedSpeechStream = polishedSpeech.repeat(streamCopies);

export function DictationSection() {
  return (
    <section id="dictation" className="pt-16 md:pt-20">
      <h2 className="text-color font-hand text-3xl leading-none font-semibold">
        <span className="font-hand opacity-45">Stop typing.</span> Start
        talking.
      </h2>
      <p className="text-color mx-auto mt-6 max-w-xl text-lg leading-8">
        Dictation, built right into Anarlog. Turn your thoughts into text in the
        apps you already use.
      </p>
      <SpeechFlowVisual />
    </section>
  );
}

function SpeechFlowVisual() {
  const id = useId();
  const reducedMotion = useReducedMotion();
  const svgRef = useRef<SVGSVGElement>(null);
  const rawRef = useRef<SVGTextElement>(null);
  const polishedRef = useRef<SVGTextElement>(null);
  const rawPathRef = useRef<SVGPathElement>(null);
  const [lengths, setLengths] = useState({ raw: 0, polished: 0, path: 0 });

  useMountEffect(() => {
    let active = true;
    const measure = () => {
      if (
        !active ||
        !rawRef.current ||
        !polishedRef.current ||
        !rawPathRef.current
      )
        return;
      const next = {
        raw: rawRef.current.getComputedTextLength() / streamCopies,
        polished: polishedRef.current.getComputedTextLength() / streamCopies,
        path: rawPathRef.current.getTotalLength(),
      };
      setLengths((previous) =>
        previous.raw === next.raw &&
        previous.polished === next.polished &&
        previous.path === next.path
          ? previous
          : next,
      );
    };
    void document.fonts.ready.then(measure);
    const observer = new ResizeObserver(measure);
    if (svgRef.current) observer.observe(svgRef.current);
    return () => {
      active = false;
      observer.disconnect();
    };
  });

  const rawStart = lengths.path - lengths.raw * 2;
  const polishedStart = -lengths.polished;
  const animated = !reducedMotion && lengths.raw > 0 && lengths.polished > 0;

  return (
    <div
      className="speech-flow-stage"
      aria-label="Rough speech becomes polished text"
    >
      <p className="sr-only">
        Rough speech: {rawSpeech}. Polished text: {polishedSpeech}
      </p>
      <svg
        ref={svgRef}
        className="speech-flow-svg"
        viewBox="0 0 1180 360"
        preserveAspectRatio="xMidYMid meet"
        aria-hidden="true"
      >
        <path
          ref={rawPathRef}
          id={`${id}-raw`}
          d="M-80 226 C 40 184 172 172 244 214 C 320 258 308 320 232 318 C 142 316 120 254 174 198 C 236 136 322 150 382 166 C 436 181 464 176 500 176"
        />
        <path
          id={`${id}-polished`}
          className="speech-flow-ribbon"
          d="M690 176 C 804 176 878 176 940 168 C 1008 158 1038 128 1078 70 C 1106 32 1138 10 1180 -4"
        />
        <text
          ref={rawRef}
          className="speech-flow-text speech-flow-text--raw"
          x={lengths.raw > 0 ? rawStart : -120}
        >
          <textPath href={`#${id}-raw`}>
            {rawSpeechStreamParts.map((part) => (
              <tspan
                key={part.id}
                className={part.filler ? "speech-flow-filler" : undefined}
              >
                {part.text}
              </tspan>
            ))}
          </textPath>
          {animated && (
            <animate
              attributeName="x"
              dur="75s"
              values={`${rawStart};${rawStart + lengths.raw}`}
              repeatCount="indefinite"
            />
          )}
        </text>
        <text
          ref={polishedRef}
          className="speech-flow-text speech-flow-text--polished"
          x={polishedStart}
        >
          <textPath href={`#${id}-polished`}>{polishedSpeechStream}</textPath>
          {animated && (
            <animate
              attributeName="x"
              begin="0.8s"
              dur="75s"
              values={`${polishedStart};0`}
              repeatCount="indefinite"
            />
          )}
        </text>
      </svg>

      <Link className="speech-flow-control" to="/download/">
        <span className="speech-flow-bars" aria-hidden="true">
          <span />
          <span />
          <span />
        </span>
        <span className="speech-flow-control__label">
          Try dictation with Pro
        </span>
      </Link>
    </div>
  );
}

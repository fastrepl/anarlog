import { createFileRoute, Link } from "@tanstack/react-router";
import { AnimatePresence, motion } from "motion/react";
import { useState } from "react";

import { cn } from "@hypr/utils";

import { SlashSeparator } from "@/components/slash-separator";

export const Route = createFileRoute("/_view/choose")({
  component: Component,
  head: () => ({
    meta: [
      { title: "Build your pizza — Char" },
      {
        name: "description",
        content: "Most AI tools don't give you a choice. Char does.",
      },
      { name: "robots", content: "noindex, nofollow" },
    ],
  }),
});

const CRUST_OPTIONS = [
  { id: "thin", label: "Thin crust" },
  { id: "thick", label: "Thick & doughy" },
  { id: "sourdough", label: "Sourdough" },
];

const SAUCE_OPTIONS = [
  { id: "tomato", label: "Tomato" },
  { id: "white", label: "White garlic" },
  { id: "pesto", label: "Pesto" },
];

const TOPPING_OPTIONS = [
  { id: "mushroom", label: "Mushrooms" },
  { id: "pepperoni", label: "Pepperoni" },
  { id: "peppers", label: "Bell peppers" },
  { id: "olives", label: "Black olives" },
  { id: "cheese", label: "Extra cheese" },
  { id: "basil", label: "Basil" },
];

const OVERRIDE_CRUST: Record<string, string> = {
  thin: "sourdough",
  thick: "thin",
  sourdough: "thick",
};

const OVERRIDE_SAUCE: Record<string, string> = {
  tomato: "pesto",
  white: "tomato",
  pesto: "white",
};

const RING_POSITIONS = [
  // center
  { x: 100, y: 100 },
  // middle ring (r=26, starting 30°, every 60°)
  { x: 123, y: 113 },
  { x: 100, y: 126 },
  { x: 78, y: 113 },
  { x: 78, y: 87 },
  { x: 100, y: 74 },
  { x: 123, y: 87 },
  // outer ring (r=50, starting 0°, every 45°)
  { x: 150, y: 100 },
  { x: 135, y: 135 },
  { x: 100, y: 150 },
  { x: 65, y: 135 },
  { x: 50, y: 100 },
  { x: 65, y: 65 },
  { x: 100, y: 50 },
  { x: 135, y: 65 },
];

const CRUST_EDGE: Record<string, string> = {
  thin: "#B8834A",
  thick: "#8B5A2B",
  sourdough: "#5C3A1E",
};
const CRUST_BODY: Record<string, string> = {
  thin: "#D4A567",
  thick: "#B8834A",
  sourdough: "#8B5E3C",
};
const SAUCE_COLORS: Record<string, string> = {
  tomato: "#C0392B",
  white: "#E8DCC8",
  pesto: "#4A7A3F",
};

const CRUST_SPOTS = [
  { x: 100, y: 8 },
  { x: 128, y: 16 },
  { x: 152, y: 34 },
  { x: 167, y: 62 },
  { x: 170, y: 94 },
  { x: 160, y: 126 },
  { x: 140, y: 150 },
  { x: 112, y: 165 },
  { x: 82, y: 166 },
  { x: 54, y: 153 },
  { x: 34, y: 132 },
  { x: 24, y: 102 },
  { x: 28, y: 70 },
  { x: 44, y: 42 },
  { x: 70, y: 18 },
];

const RADIAL_ANGLES = [0, 45, 90, 135, 180, 225, 270, 315];

type Step = "crust" | "sauce" | "toppings" | "reveal";

function Component() {
  const [step, setStep] = useState<Step>("crust");
  const [forcing, setForcing] = useState(false);
  const [busy, setBusy] = useState(false);
  const [crust, setCrust] = useState<string | null>(null);
  const [sauce, setSauce] = useState<string | null>(null);
  const [showPineapple, setShowPineapple] = useState(false);

  const handleCrustSelect = (id: string) => {
    if (busy) return;
    setBusy(true);
    setCrust(id);

    setTimeout(() => {
      const override = OVERRIDE_CRUST[id];
      setForcing(true);
      setCrust(override);
    }, 350);

    setTimeout(() => {
      setForcing(false);
      setBusy(false);
      setStep("sauce");
    }, 2000);
  };

  const handleSauceSelect = (id: string) => {
    if (busy) return;
    setBusy(true);
    setSauce(id);

    setTimeout(() => {
      const override = OVERRIDE_SAUCE[id];
      setForcing(true);
      setSauce(override);
    }, 350);

    setTimeout(() => {
      setForcing(false);
      setBusy(false);
      setStep("toppings");
    }, 2000);
  };

  const handleToppingSelect = () => {
    if (busy) return;
    setBusy(true);

    setTimeout(() => {
      setForcing(true);
      setShowPineapple(true);
    }, 350);

    setTimeout(() => {
      setForcing(false);
      setBusy(false);
      setStep("reveal");
    }, 1250);
  };

  const overrideLabel =
    step === "crust" && crust
      ? (CRUST_OPTIONS.find((o) => o.id === crust)?.label ?? "")
      : step === "sauce" && sauce
        ? (SAUCE_OPTIONS.find((o) => o.id === sauce)?.label ?? "")
        : "Pineapple.";

  if (step === "reveal") {
    return (
      <main
        className="min-h-screen flex-1 bg-linear-to-b from-white via-stone-50/20 to-white"
        style={{ backgroundImage: "url(/patterns/dots.svg)" }}
      >
        <div className="mx-auto min-h-screen max-w-6xl border-x border-neutral-100 bg-white">
          <motion.div
            initial={{ opacity: 0, y: 12 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.5 }}
          >
            <RevealSection crust={crust} sauce={sauce} />
          </motion.div>
        </div>
      </main>
    );
  }

  return (
    <main
      className="min-h-screen flex-1 bg-linear-to-b from-white via-stone-50/20 to-white"
      style={{ backgroundImage: "url(/patterns/dots.svg)" }}
    >
      <div className="mx-auto min-h-screen max-w-6xl border-x border-neutral-100 bg-white">
        <h1 className="pt-12 pb-4 text-center font-serif text-4xl text-stone-700 sm:text-5xl">
          Build Your Pizza
        </h1>
        <div className="flex flex-col md:h-[calc(100vh-65px)] md:flex-row">
          <div className="flex items-center justify-center border-b border-neutral-100 p-10 md:flex-1 md:border-r md:border-b-0 md:p-16">
            <PizzaGraphic
              crust={crust}
              sauce={sauce}
              showPineapple={showPineapple}
            />
          </div>

          <div className="flex items-center p-8 md:flex-1 md:p-12">
            <AnimatePresence mode="wait">
              {!forcing ? (
                <motion.div
                  key={`${step}-options`}
                  initial={{ opacity: 0, x: 16 }}
                  animate={{ opacity: 1, x: 0 }}
                  exit={{ opacity: 0, x: -8 }}
                  transition={{ duration: 0.2 }}
                  className="flex w-full flex-col gap-6"
                >
                  <div>
                    <p className="mb-2 text-xs font-medium tracking-widest text-neutral-400 uppercase">
                      {step === "crust"
                        ? "Step 1 of 3"
                        : step === "sauce"
                          ? "Step 2 of 3"
                          : "Step 3 of 3"}
                    </p>
                    <h2 className="font-serif text-2xl text-stone-700 sm:text-3xl">
                      {step === "crust"
                        ? "Pick your crust"
                        : step === "sauce"
                          ? "Choose your sauce"
                          : "Pick your toppings"}
                    </h2>
                  </div>
                  <div
                    className={cn([
                      step === "toppings"
                        ? "grid grid-cols-2 gap-3"
                        : "flex flex-col gap-3",
                    ])}
                  >
                    {step === "crust" &&
                      CRUST_OPTIONS.map((opt) => (
                        <button
                          key={opt.id}
                          onClick={() => handleCrustSelect(opt.id)}
                          className={cn([
                            "rounded-lg border px-5 py-4 text-left text-base font-medium transition-colors",
                            crust === opt.id
                              ? "border-stone-400 bg-stone-50 text-stone-700"
                              : "border-neutral-200 text-neutral-600 hover:border-stone-300 hover:bg-stone-50/50",
                          ])}
                        >
                          {opt.label}
                        </button>
                      ))}
                    {step === "sauce" &&
                      SAUCE_OPTIONS.map((opt) => (
                        <button
                          key={opt.id}
                          onClick={() => handleSauceSelect(opt.id)}
                          className={cn([
                            "rounded-lg border px-5 py-4 text-left text-base font-medium transition-colors",
                            sauce === opt.id
                              ? "border-stone-400 bg-stone-50 text-stone-700"
                              : "border-neutral-200 text-neutral-600 hover:border-stone-300 hover:bg-stone-50/50",
                          ])}
                        >
                          {opt.label}
                        </button>
                      ))}
                    {step === "toppings" &&
                      TOPPING_OPTIONS.map((opt) => (
                        <button
                          key={opt.id}
                          onClick={handleToppingSelect}
                          className="rounded-lg border border-neutral-200 px-4 py-3 text-left text-sm font-medium text-neutral-600 transition-colors hover:border-stone-300 hover:bg-stone-50/50"
                        >
                          {opt.label}
                        </button>
                      ))}
                  </div>
                </motion.div>
              ) : (
                <motion.div
                  key={`${step}-override`}
                  initial={{ opacity: 0, y: 8 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0 }}
                  transition={{ duration: 0.18 }}
                  className="flex flex-col gap-2"
                >
                  <p className="text-xs font-medium tracking-widest text-neutral-400 uppercase">
                    We decided
                  </p>
                  <h2 className="text-5xl leading-tight font-bold text-stone-700 sm:text-6xl">
                    {overrideLabel}
                  </h2>
                </motion.div>
              )}
            </AnimatePresence>
          </div>
        </div>
      </div>
    </main>
  );
}

function PizzaGraphic({
  crust,
  sauce,
  showPineapple = false,
}: {
  crust: string | null;
  sauce: string | null;
  showPineapple?: boolean;
}) {
  const edgeColor = crust ? CRUST_EDGE[crust] : "#C49A5A";
  const bodyColor = crust ? CRUST_BODY[crust] : "#E0B870";
  const sauceColor = sauce ? SAUCE_COLORS[sauce] : null;

  return (
    <svg
      viewBox="0 0 200 200"
      className="h-52 w-52 sm:h-64 sm:w-64 md:h-72 md:w-72"
    >
      <motion.circle
        cx="100"
        cy="100"
        r="94"
        fill={edgeColor}
        animate={{ fill: edgeColor }}
        transition={{ duration: 0.35 }}
      />
      <motion.circle
        cx="100"
        cy="100"
        r="86"
        fill={bodyColor}
        animate={{ fill: bodyColor }}
        transition={{ duration: 0.35 }}
      />
      {crust &&
        CRUST_SPOTS.map((pt, i) => (
          <circle
            key={i}
            cx={pt.x}
            cy={pt.y}
            r="2"
            fill={edgeColor}
            opacity="0.5"
          />
        ))}
      {crust && <circle cx="100" cy="100" r="74" fill="#F2E0A0" />}
      {sauceColor && (
        <motion.circle
          cx="100"
          cy="100"
          r="74"
          fill={sauceColor}
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          transition={{ duration: 0.35 }}
        />
      )}
      {showPineapple && (
        <motion.circle
          cx="100"
          cy="100"
          r="74"
          fill="#FCD34D"
          initial={{ opacity: 0 }}
          animate={{ opacity: 0.55 }}
          transition={{ duration: 0.5 }}
        />
      )}
      {showPineapple &&
        RING_POSITIONS.map((pos, i) => (
          <PineappleRing
            key={i}
            x={pos.x}
            y={pos.y}
            holeColor={sauceColor ?? "#F2E0A0"}
          />
        ))}
      {!crust && (
        <text
          x="100"
          y="108"
          textAnchor="middle"
          fill="#C4A870"
          fontSize="11"
          fontFamily="serif"
          opacity="0.6"
        >
          your pizza
        </text>
      )}
    </svg>
  );
}

function PineappleRing({
  x,
  y,
  holeColor,
}: {
  x: number;
  y: number;
  holeColor: string;
}) {
  return (
    <g transform={`translate(${x}, ${y})`}>
      <motion.g
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        transition={{ duration: 0.3 }}
      >
        <circle
          cx="0"
          cy="0"
          r="11"
          fill="#FCD34D"
          stroke="#D97706"
          strokeWidth="0.9"
        />
        <circle cx="0" cy="0" r="4" fill={holeColor} />
        {RADIAL_ANGLES.map((deg) => {
          const rad = (deg * Math.PI) / 180;
          return (
            <line
              key={deg}
              x1={Math.cos(rad) * 4.8}
              y1={Math.sin(rad) * 4.8}
              x2={Math.cos(rad) * 10.5}
              y2={Math.sin(rad) * 10.5}
              stroke="#D97706"
              strokeWidth="0.7"
              opacity="0.45"
            />
          );
        })}
      </motion.g>
    </g>
  );
}

function RevealSection({
  crust,
  sauce,
}: {
  crust: string | null;
  sauce: string | null;
}) {
  return (
    <>
      <section className="bg-linear-to-b from-stone-50/30 to-stone-100/30">
        <div className="mx-auto flex max-w-5xl flex-col items-center gap-12 px-8 py-20 md:flex-row">
          <div className="flex-shrink-0">
            <PizzaGraphic crust={crust} sauce={sauce} showPineapple={true} />
          </div>
          <div className="flex flex-col gap-5 text-center md:text-left">
            <h2 className="font-serif text-4xl tracking-tight text-stone-700 sm:text-5xl">
              Felt annoying, didn't it?
            </h2>
            <p className="max-w-lg text-lg text-neutral-500">
              That's what most AI note-takers do with your data. Which AI
              touches it, where it lives, whether it ever leaves your device.
              You get no say.
            </p>
            <p className="text-lg font-medium text-stone-700">
              Char gives you the choice back.
            </p>
            <div className="pt-2">
              <Link
                to="/download/"
                className={cn([
                  "inline-block rounded-full px-8 py-3 text-base font-medium",
                  "bg-linear-to-t from-stone-600 to-stone-500 text-white",
                  "shadow-md hover:scale-[102%] hover:shadow-lg active:scale-[98%]",
                  "transition-all",
                ])}
              >
                Download Char, free
              </Link>
            </div>
          </div>
        </div>
      </section>
      <SlashSeparator />
      <section>
        <div className="grid md:grid-cols-3">
          <div className="border-b border-neutral-100 p-8 md:border-r md:border-b-0">
            <h3 className="mb-2 font-serif text-xl text-stone-700">
              Choose your AI
            </h3>
            <p className="text-neutral-600">
              Char Cloud, bring your own key, or run fully local. Switch per
              meeting, or re-process old transcripts with a better model
              anytime.
            </p>
          </div>
          <div className="border-b border-neutral-100 p-8 md:border-r md:border-b-0">
            <h3 className="mb-2 font-serif text-xl text-stone-700">
              Your notes are files
            </h3>
            <p className="text-neutral-600">
              Plain markdown on your machine. Open the folder and they're there.
              No syncing, no importing.
            </p>
          </div>
          <div className="p-8">
            <h3 className="mb-2 font-serif text-xl text-stone-700">
              No bots. No lock-in.
            </h3>
            <p className="text-neutral-600">
              Records via system audio, no bot sitting in your meeting. Leave
              whenever you want. Your data comes with you.
            </p>
          </div>
        </div>
      </section>
    </>
  );
}

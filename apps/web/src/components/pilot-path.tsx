import { pilotSteps } from "@/lib/trust-center";

export function PilotPath() {
  return (
    <ol className="mx-auto mt-8 flex max-w-2xl flex-col gap-6 text-left">
      {pilotSteps.map((step, index) => (
        <li key={step.title} className="flex gap-4">
          <span className="mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-[#181613] text-xs font-medium text-white">
            {index + 1}
          </span>
          <div>
            <h3 className="text-base font-medium text-[#181613]">
              {step.title}
            </h3>
            <p className="mt-1 text-sm leading-6 text-[#4f4940]">{step.body}</p>
          </div>
        </li>
      ))}
    </ol>
  );
}

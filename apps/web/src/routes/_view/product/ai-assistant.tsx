import { Icon } from "@iconify-icon/react";
import { createFileRoute, Link } from "@tanstack/react-router";
import { useEffect, useState } from "react";

import { cn } from "@hypr/utils";

import { SlashSeparator } from "@/components/slash-separator";

export const Route = createFileRoute("/_view/product/ai-assistant")({
  component: Component,
  head: () => ({
    meta: [
      { title: "AI Chat - Char" },
      {
        name: "description",
        content:
          "AI assistant that helps you before, during, and after meetings. Prepare with research, get realtime insights, and execute workflows—all powered by local AI.",
      },
      { name: "robots", content: "noindex, nofollow" },
    ],
  }),
});

function Component() {
  return (
    <div
      className="min-h-screen bg-linear-to-b from-white via-stone-50/20 to-white"
      style={{ backgroundImage: "url(/patterns/dots.svg)" }}
    >
      <div className="mx-auto max-w-6xl border-x border-neutral-100 bg-white">
        <HeroSection />
        <SlashSeparator />
        <BeforeMeetingSection />
        <SlashSeparator />
        <DuringMeetingSection />
        <SlashSeparator />
        <AfterMeetingSection />
        <SlashSeparator />
        <CTASection />
      </div>
    </div>
  );
}

function HeroSection() {
  return (
    <div className="bg-linear-to-b from-stone-50/30 to-stone-100/30 px-6 py-12 lg:py-20">
      <header className="mx-auto max-w-4xl text-center">
        <h1 className="mb-6 flex flex-wrap items-center justify-center font-serif text-4xl tracking-tight text-stone-700 sm:text-5xl">
          <span>AI Chat</span>
          <img
            src="/api/images/hyprnote/ai-assistant.gif"
            alt="AI Chat"
            className="mr-2 ml-1 inline-block h-12 w-12 rounded-full object-cover sm:mr-0 sm:h-16 sm:w-16"
          />
          <span>for your meetings</span>
        </h1>
        <p className="text-lg text-neutral-600 sm:text-xl">
          Prepare, engage, and follow through with AI-powered assistance
        </p>
        <div className="mt-8">
          <Link
            to="/download/"
            className={cn([
              "inline-block rounded-full px-8 py-3 text-base font-medium",
              "bg-linear-to-t from-stone-600 to-stone-500 text-white",
              "transition-transform hover:scale-105 active:scale-95",
            ])}
          >
            Download for free
          </Link>
        </div>
      </header>
    </div>
  );
}

function BeforeMeetingSection() {
  return (
    <section id="before-meeting">
      <div className="py-6 text-center font-serif font-medium tracking-wide text-neutral-600 uppercase">
        Before meetings
      </div>

      <div className="border-t border-neutral-100">
        <div className="grid md:grid-cols-2">
          <div className="border-b border-neutral-100 p-8 md:border-r md:border-b-0">
            <Icon icon="mdi:magnify" className="mb-4 text-3xl text-stone-700" />
            <h3 className="mb-3 font-serif text-xl text-stone-700">
              Deep research with chat
            </h3>
            <p className="mb-4 leading-relaxed text-neutral-600">
              Chat with your AI assistant to learn more about the people you're
              meeting with. Search through past conversations, extract key
              insights, and understand context before you join.
            </p>
            <ul className="flex flex-col gap-2">
              <li className="flex items-start gap-2">
                <Icon
                  icon="mdi:check"
                  className="mt-0.5 shrink-0 text-stone-700"
                />
                <span className="text-sm text-neutral-600">
                  "What did we discuss last time with Sarah?"
                </span>
              </li>
              <li className="flex items-start gap-2">
                <Icon
                  icon="mdi:check"
                  className="mt-0.5 shrink-0 text-stone-700"
                />
                <span className="text-sm text-neutral-600">
                  "What are the client's main concerns?"
                </span>
              </li>
              <li className="flex items-start gap-2">
                <Icon
                  icon="mdi:check"
                  className="mt-0.5 shrink-0 text-stone-700"
                />
                <span className="text-sm text-neutral-600">
                  "Show me all action items from previous meetings"
                </span>
              </li>
            </ul>
          </div>

          <div className="border-b border-neutral-100 p-8 md:border-b-0">
            <Icon
              icon="mdi:file-document-edit"
              className="mb-4 text-3xl text-stone-700"
            />
            <h3 className="mb-3 font-serif text-xl text-stone-700">
              Generate custom templates
            </h3>
            <p className="mb-4 leading-relaxed text-neutral-600">
              Create tailored meeting templates on the spot. Ask your AI
              assistant to generate agendas, question lists, or note structures
              specific to your meeting type.
            </p>
            <ul className="flex flex-col gap-2">
              <li className="flex items-start gap-2">
                <Icon
                  icon="mdi:check"
                  className="mt-0.5 shrink-0 text-stone-700"
                />
                <span className="text-sm text-neutral-600">
                  "Create a customer discovery template"
                </span>
              </li>
              <li className="flex items-start gap-2">
                <Icon
                  icon="mdi:check"
                  className="mt-0.5 shrink-0 text-stone-700"
                />
                <span className="text-sm text-neutral-600">
                  "Generate questions for a technical interview"
                </span>
              </li>
              <li className="flex items-start gap-2">
                <Icon
                  icon="mdi:check"
                  className="mt-0.5 shrink-0 text-stone-700"
                />
                <span className="text-sm text-neutral-600">
                  "Build an agenda for our quarterly review"
                </span>
              </li>
            </ul>
          </div>
        </div>

        <div className="border-t border-neutral-100 p-8">
          <Icon
            icon="mdi:message-question"
            className="mb-4 text-3xl text-stone-700"
          />
          <h3 className="mb-3 font-serif text-xl text-stone-700">
            Ask about past conversations
          </h3>
          <p className="max-w-3xl leading-relaxed text-neutral-600">
            Query your entire conversation history to refresh your memory. Find
            decisions, action items, or specific topics discussed in previous
            meetings—all in natural language.
          </p>
        </div>
      </div>
    </section>
  );
}

function DuringMeetingSection() {
  return (
    <section id="during-meeting">
      <div className="py-6 text-center font-serif font-medium tracking-wide text-neutral-600 uppercase">
        During meetings
      </div>

      <div className="border-t border-neutral-100">
        <div className="grid md:grid-cols-2">
          <div className="border-b border-neutral-100 p-8 md:border-r md:border-b-0">
            <Icon icon="mdi:chat" className="mb-4 text-3xl text-stone-700" />
            <h3 className="mb-3 font-serif text-xl text-stone-700">
              Ask questions in realtime
            </h3>
            <p className="leading-relaxed text-neutral-600">
              Type questions to your AI assistant during the meeting without
              interrupting the conversation. Get instant answers from the
              current transcript and past meeting context.
            </p>
          </div>

          <div className="border-b border-neutral-100 p-8 md:border-b-0">
            <Icon
              icon="mdi:lightbulb-on"
              className="mb-4 text-3xl text-stone-700"
            />
            <h3 className="mb-3 font-serif text-xl text-stone-700">
              Realtime insights via{" "}
              <Link
                to="/product/extensions/"
                className="text-stone-700 underline decoration-dotted underline-offset-2 hover:text-stone-800"
              >
                extensions
              </Link>
            </h3>
            <p className="mb-4 leading-relaxed text-neutral-600">
              AI-powered extensions provide live assistance during your meeting.
              Built on our extension framework, these tools adapt to your needs
              in realtime.
            </p>
          </div>
        </div>

        <div className="border-t border-neutral-100">
          <div className="px-6 py-8 lg:px-8">
            <h4 className="mb-6 text-center font-serif text-lg text-stone-700">
              Available realtime extensions
            </h4>
            <div className="grid gap-6 md:grid-cols-3">
              <div className="rounded-lg border border-neutral-200 bg-stone-50 p-6">
                <Icon
                  icon="mdi:comment-check"
                  className="mb-3 text-2xl text-stone-700"
                />
                <h5 className="mb-2 font-medium text-stone-700">Suggestions</h5>
                <p className="text-sm text-neutral-600">
                  Get AI-generated advice and recommendations based on the
                  conversation flow
                </p>
              </div>

              <div className="rounded-lg border border-neutral-200 bg-stone-50 p-6">
                <Icon
                  icon="mdi:account-voice"
                  className="mb-3 text-2xl text-stone-700"
                />
                <h5 className="mb-2 font-medium text-stone-700">
                  Talk time tracking
                </h5>
                <p className="text-sm text-neutral-600">
                  Monitor who's speaking and for how long to ensure balanced
                  participation
                </p>
              </div>

              <div className="rounded-lg border border-neutral-200 bg-stone-50 p-6">
                <Icon
                  icon="mdi:school"
                  className="mb-3 text-2xl text-stone-700"
                />
                <h5 className="mb-2 font-medium text-stone-700">
                  ELI5 explanations
                </h5>
                <p className="text-sm text-neutral-600">
                  Get instant simple explanations of technical or professional
                  jargon
                </p>
              </div>
            </div>

            <div className="mt-6 text-center">
              <Link
                to="/product/extensions/"
                className="inline-flex items-center gap-2 font-medium text-stone-700 hover:text-stone-800"
              >
                Learn more about extensions
                <Icon icon="mdi:arrow-right" className="text-lg" />
              </Link>
            </div>
          </div>
        </div>
      </div>
    </section>
  );
}

function AfterMeetingSection() {
  const slides = [
    {
      prompt:
        "Add a Jira ticket for the mobile UI bug and assign it to Sarah today",
      card: (
        <div
          className="w-full max-w-[420px] rounded-2xl border border-neutral-200 bg-white p-4"
          dir="ltr"
        >
          <div className="flex flex-row items-center gap-2 text-xs text-neutral-500">
            <Icon icon="logos:jira" className="text-base" />
            <span>ENG-247</span>
            <span className="rounded-full bg-stone-100 px-2 py-0.5 text-[11px] text-stone-700">
              Todo
            </span>
          </div>
          <div className="mt-2 text-sm font-medium text-stone-700">
            Mobile UI bug fix
          </div>
          <p className="mt-1 text-xs text-neutral-500">
            Fix the mobile UI bug discussed in today's meeting. Check responsive
            layout on iOS devices.
          </p>
          <div className="mt-3 flex flex-row items-center gap-2 text-xs text-neutral-500">
            <div className="flex h-6 w-6 items-center justify-center rounded-full bg-amber-500 text-[10px] text-white">
              S
            </div>
            <span>Sarah</span>
          </div>
        </div>
      ),
      toolbar: "simple-icons:jira",
    },
    {
      prompt: "Send the summary to #engineering and update the Q4 roadmap now",
      card: (
        <div
          className="mt-1 w-full max-w-[420px] rounded-[18px] border border-neutral-200 bg-white px-4 py-4 text-sm text-neutral-700"
          dir="ltr"
        >
          <div className="flex items-center gap-2 text-xs text-neutral-500">
            <Icon icon="logos:slack-icon" className="text-sm" />
            <span>#engineering</span>
            <span>·</span>
            <span>2:15 PM</span>
          </div>
          <div className="mt-2">
            <p className="font-medium text-neutral-700">Jessica Lee</p>
            <p className="text-[12px] text-neutral-600">
              Meeting summary attached as a file for review, including key
              decisions, action items, and next steps for the Q4 rollout.
            </p>
            <div className="mt-2 inline-flex items-center gap-2 rounded-md border border-neutral-200 bg-white px-2 py-1 text-xs text-neutral-600">
              <Icon icon="mdi:file-outline" className="text-sm" />
              <span>meeting-summary.pdf</span>
            </div>
          </div>
        </div>
      ),
      toolbar: "simple-icons:slack",
    },
    {
      prompt:
        "Schedule a follow-up next week with the client and share the agenda",
      card: (
        <div
          className="mt-1 w-full max-w-[420px] rounded-[18px] border border-neutral-200 bg-white px-4 py-4 text-sm text-neutral-700"
          dir="ltr"
        >
          <div className="flex items-center gap-2 text-xs text-neutral-500">
            <Icon icon="logos:google-calendar" className="text-sm" />
            <span>Mon, 9:30 AM</span>
            <span>·</span>
            <span>30 min</span>
          </div>
          <div className="mt-2">
            <p className="font-medium text-neutral-700">Follow-up meeting</p>
            <p className="text-xs text-neutral-600">
              2 guests · 1 yes, 1 awaiting
            </p>
            <div className="mt-2 flex items-center gap-2 text-xs text-neutral-500">
              <div className="flex size-5 items-center justify-center rounded-full bg-stone-200 text-[10px] text-stone-700">
                A
              </div>
              <span>John Smith</span>
            </div>
            <div className="mt-1 flex items-center gap-2 text-xs text-neutral-500">
              <div className="flex size-5 items-center justify-center rounded-full bg-amber-500 text-[10px] text-white">
                M
              </div>
              <span>Mudit Jain</span>
            </div>
          </div>
        </div>
      ),
      toolbar: "simple-icons:googlecalendar",
    },
  ];
  const [activeIndex, setActiveIndex] = useState(0);
  const [progress, setProgress] = useState(0);

  useEffect(() => {
    const interval = window.setInterval(() => {
      setProgress((current) => {
        const next = current + 2;
        if (next >= 100) {
          setActiveIndex(
            (prevIndex) => (prevIndex - 1 + slides.length) % slides.length,
          );
          return 0;
        }
        return next;
      });
    }, 80);

    return () => window.clearInterval(interval);
  }, [slides.length]);

  const activeSlide = slides[activeIndex];

  return (
    <section id="after-meeting" dir="ltr">
      <div className="py-6 text-center font-serif font-medium tracking-wide text-neutral-600 uppercase">
        After meetings
      </div>

      <div className="border-t border-neutral-100">
        <div className="grid md:grid-cols-2">
          <div className="border-b border-neutral-100 p-8 md:border-r md:border-b-0">
            <Icon
              icon="mdi:workflow"
              className="mb-4 text-3xl text-stone-700"
            />
            <h3 className="mb-3 font-serif text-xl text-stone-700">
              Execute workflows with natural language
            </h3>
            <p className="mb-4 leading-relaxed text-neutral-600">
              Describe what you want to do, and let your AI assistant handle the
              rest. Automate follow-up tasks across your tools without manual
              data entry.
            </p>
            <div className="flex flex-col gap-6">
              <div className="flex h-[236px] flex-col items-center gap-1">
                <div
                  className="max-w-[420px] rounded-[18px] border border-neutral-200 bg-neutral-50 px-4 py-2 text-center text-sm text-neutral-700"
                  style={{
                    fontFamily:
                      '"SF Pro", -apple-system, BlinkMacSystemFont, sans-serif',
                  }}
                >
                  "{activeSlide.prompt}"
                </div>
                <div className="h-5 w-px border-l-2 border-dotted border-neutral-300" />
                <div className="flex w-full justify-center" dir="ltr">
                  {activeSlide.card}
                </div>
              </div>
              <div
                className="flex flex-row items-center justify-center gap-2 text-center"
                dir="ltr"
              >
                {slides.map((slide, index) => (
                  <button
                    key={slide.prompt}
                    type="button"
                    className="flex h-10 min-w-12 items-center justify-center rounded-[12px] border border-neutral-200 bg-stone-50 px-2 text-neutral-600 transition-colors hover:text-stone-700"
                    onClick={() => {
                      setProgress(0);
                      setActiveIndex(index);
                    }}
                    style={{
                      background:
                        index === activeIndex
                          ? `linear-gradient(90deg, rgba(0,0,0,0.08) ${progress}%, rgba(0,0,0,0.02) ${progress}%)`
                          : undefined,
                    }}
                  >
                    <Icon
                      icon={slide.toolbar}
                      className="text-base"
                      color="currentColor"
                    />
                  </button>
                ))}
              </div>
            </div>
          </div>

          <div className="border-b border-neutral-100 p-8 md:border-b-0">
            <Icon icon="mdi:brain" className="mb-4 text-3xl text-stone-700" />
            <h3 className="mb-3 font-serif text-xl text-stone-700">
              Learns and adapts with memory
            </h3>
            <p className="leading-relaxed text-neutral-600">
              Your AI assistant builds memory from your interactions. It
              remembers preferences, learns from edits you make to summaries,
              and continuously improves its assistance based on your patterns.
            </p>
            <ul className="mt-4 flex flex-col gap-3">
              <li className="flex items-start gap-3">
                <Icon
                  icon="mdi:check-circle"
                  className="mt-0.5 shrink-0 text-stone-700"
                />
                <span className="text-sm text-neutral-600">
                  Remembers your meeting preferences and formats
                </span>
              </li>
              <li className="flex items-start gap-3">
                <Icon
                  icon="mdi:check-circle"
                  className="mt-0.5 shrink-0 text-stone-700"
                />
                <span className="text-sm text-neutral-600">
                  Learns from your edits to improve future summaries
                </span>
              </li>
              <li className="flex items-start gap-3">
                <Icon
                  icon="mdi:check-circle"
                  className="mt-0.5 shrink-0 text-stone-700"
                />
                <span className="text-sm text-neutral-600">
                  Adapts to your workflow and tool preferences
                </span>
              </li>
              <li className="flex items-start gap-3">
                <Icon
                  icon="mdi:check-circle"
                  className="mt-0.5 shrink-0 text-stone-700"
                />
                <span className="text-sm text-neutral-600">
                  Builds context about your team and projects over time
                </span>
              </li>
            </ul>
          </div>
        </div>
      </div>
    </section>
  );
}

function CTASection() {
  return (
    <section className="bg-linear-to-t from-stone-50/30 to-stone-100/30 px-4 py-16 lg:px-0">
      <div className="flex flex-col items-center gap-6 text-center">
        <div className="mb-4 flex size-40 items-center justify-center rounded-[48px] border border-neutral-100 bg-transparent shadow-2xl">
          <img
            src="/api/images/hyprnote/icon.png"
            alt="Char"
            width={144}
            height={144}
            className="mx-auto size-36 rounded-[40px] border border-neutral-100"
          />
        </div>
        <h2 className="font-serif text-2xl sm:text-3xl">
          Start using your AI assistant
        </h2>
        <p className="mx-auto max-w-2xl text-lg text-neutral-600">
          Get AI-powered help before, during, and after every meeting with Char
        </p>
        <div className="flex flex-col items-center justify-center gap-4 pt-6 sm:flex-row">
          <Link
            to="/download/"
            className={cn([
              "group flex h-12 items-center justify-center px-6 text-base sm:text-lg",
              "rounded-full bg-linear-to-t from-stone-600 to-stone-500 text-white",
              "shadow-md hover:scale-[102%] hover:shadow-lg active:scale-[98%]",
              "transition-all",
            ])}
          >
            Download for free
            <svg
              xmlns="http://www.w3.org/2000/svg"
              fill="none"
              viewBox="0 0 24 24"
              strokeWidth="1.5"
              stroke="currentColor"
              className="ml-2 h-5 w-5 transition-transform group-hover:translate-x-1"
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="m12.75 15 3-3m0 0-3-3m3 3h-7.5M21 12a9 9 0 1 1-18 0 9 9 0 0 1 18 0Z"
              />
            </svg>
          </Link>
          <Link
            to="/product/ai-notetaking/"
            className={cn([
              "flex h-12 items-center justify-center px-6 text-base sm:text-lg",
              "rounded-full border border-neutral-300 text-stone-700",
              "transition-colors hover:bg-white",
            ])}
          >
            Learn about AI Notetaking
          </Link>
        </div>
      </div>
    </section>
  );
}

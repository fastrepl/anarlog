import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";

describe("OnboardingSurveyPrompt", () => {
  test("opens on the second launch and submits ID-based responses", async () => {
    vi.resetModules();

    const { commands: tauriCommands } = await import("~/types/tauri.gen");
    const { commands: analyticsCommands } =
      await import("@hypr/plugin-analytics");
    const { OnboardingSurveyPrompt } = await import("./prompt");

    vi.mocked(tauriCommands.recordOnboardingSurveyLaunch).mockResolvedValue({
      status: "ok",
      data: { launchCount: 2, done: false },
    });
    vi.mocked(tauriCommands.finishOnboardingSurvey).mockResolvedValue({
      status: "ok",
      data: { launchCount: 2, done: true },
    });

    render(
      <QueryClientProvider client={new QueryClient()}>
        <OnboardingSurveyPrompt />
      </QueryClientProvider>,
    );

    await screen.findByText("How did you find us?");
    await waitFor(() => {
      expect(analyticsCommands.event).toHaveBeenCalledWith({
        event: "survey shown",
        $survey_id: "019d7b82-451a-0000-e4c8-3a53dd3f2435",
      });
    });

    fireEvent.click(
      screen.getByRole("button", {
        name: "Search engine (Google, Bing, etc)",
      }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Next" }));

    fireEvent.click(
      screen.getByRole("button", {
        name: "I want to choose my own AI provider",
      }),
    );
    fireEvent.click(screen.getByRole("button", { name: "It's open source" }));
    fireEvent.click(screen.getByRole("button", { name: "Next" }));

    fireEvent.click(
      screen.getByRole("button", { name: "Founder / Executive" }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Next" }));

    fireEvent.click(
      screen.getByRole("button", {
        name: "AI tool without a bot (Granola, Jamie, etc.)",
      }),
    );
    fireEvent.click(screen.getByRole("button", { name: "Submit" }));

    await waitFor(() => {
      expect(analyticsCommands.event).toHaveBeenCalledWith({
        event: "survey sent",
        $survey_id: "019d7b82-451a-0000-e4c8-3a53dd3f2435",
        $survey_questions: [
          {
            id: "7405e454-508b-4e2a-80a7-e04e84f0bbaa",
            question: "How did you find us?",
            response: "Search engine (Google, Bing, etc)",
          },
          {
            id: "a9b47320-a169-4f93-90e6-15375fed4e8d",
            question: "Why did you decide to use Char?",
            response: [
              "I want to choose my own AI provider",
              "It's open source",
            ],
          },
          {
            id: "ce59d931-ed0d-4d23-9ab5-55656a1e638a",
            question: "What best describes your role?",
            response: "Founder / Executive",
          },
          {
            id: "d58b62d0-c689-4c72-877d-fa949b30ca47",
            question: "How have you been taking notes?",
            response: ["AI tool without a bot (Granola, Jamie, etc.)"],
          },
        ],
        "$survey_response_7405e454-508b-4e2a-80a7-e04e84f0bbaa":
          "Search engine (Google, Bing, etc)",
        "$survey_response_a9b47320-a169-4f93-90e6-15375fed4e8d": [
          "I want to choose my own AI provider",
          "It's open source",
        ],
        "$survey_response_ce59d931-ed0d-4d23-9ab5-55656a1e638a":
          "Founder / Executive",
        "$survey_response_d58b62d0-c689-4c72-877d-fa949b30ca47": [
          "AI tool without a bot (Granola, Jamie, etc.)",
        ],
      });
    });
    expect(tauriCommands.finishOnboardingSurvey).toHaveBeenCalledTimes(1);
  });

  test("submits typed open-choice responses instead of the Other label", async () => {
    vi.resetModules();

    const { commands: tauriCommands } = await import("~/types/tauri.gen");
    const { commands: analyticsCommands } =
      await import("@hypr/plugin-analytics");
    const { OnboardingSurveyPrompt } = await import("./prompt");

    vi.mocked(tauriCommands.recordOnboardingSurveyLaunch).mockResolvedValue({
      status: "ok",
      data: { launchCount: 2, done: false },
    });
    vi.mocked(tauriCommands.finishOnboardingSurvey).mockResolvedValue({
      status: "ok",
      data: { launchCount: 2, done: true },
    });

    render(
      <QueryClientProvider client={new QueryClient()}>
        <OnboardingSurveyPrompt />
      </QueryClientProvider>,
    );

    await screen.findByText("How did you find us?");

    fireEvent.click(screen.getByRole("button", { name: "Other" }));
    const discoveryOtherInput = screen.getByRole("textbox", {
      name: "How did you find us? other response",
    });
    expect(screen.queryByRole("button", { name: "Other" })).toBeNull();
    expect(discoveryOtherInput.getAttribute("placeholder")).toBe(
      "Please specify",
    );
    expect(document.activeElement).toBe(discoveryOtherInput);
    fireEvent.change(discoveryOtherInput, {
      target: { value: "Newsletter" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Next" }));

    fireEvent.click(screen.getByRole("button", { name: "It's open source" }));
    fireEvent.click(screen.getByRole("button", { name: "Other" }));
    fireEvent.change(
      screen.getByRole("textbox", {
        name: "Why did you decide to use Char? other response",
      }),
      {
        target: { value: "No meeting bot" },
      },
    );
    fireEvent.click(screen.getByRole("button", { name: "Next" }));

    fireEvent.click(screen.getByRole("button", { name: "Other" }));
    fireEvent.change(
      screen.getByRole("textbox", {
        name: "What best describes your role? other response",
      }),
      {
        target: { value: "Attorney" },
      },
    );
    fireEvent.click(screen.getByRole("button", { name: "Next" }));

    fireEvent.click(screen.getByRole("button", { name: "Other" }));
    fireEvent.change(
      screen.getByRole("textbox", {
        name: "How have you been taking notes? other response",
      }),
      {
        target: { value: "Voice memos" },
      },
    );
    fireEvent.click(screen.getByRole("button", { name: "Submit" }));

    await waitFor(() => {
      expect(analyticsCommands.event).toHaveBeenCalledWith({
        event: "survey sent",
        $survey_id: "019d7b82-451a-0000-e4c8-3a53dd3f2435",
        $survey_questions: [
          {
            id: "7405e454-508b-4e2a-80a7-e04e84f0bbaa",
            question: "How did you find us?",
            response: "Newsletter",
          },
          {
            id: "a9b47320-a169-4f93-90e6-15375fed4e8d",
            question: "Why did you decide to use Char?",
            response: ["It's open source", "No meeting bot"],
          },
          {
            id: "ce59d931-ed0d-4d23-9ab5-55656a1e638a",
            question: "What best describes your role?",
            response: "Attorney",
          },
          {
            id: "d58b62d0-c689-4c72-877d-fa949b30ca47",
            question: "How have you been taking notes?",
            response: ["Voice memos"],
          },
        ],
        "$survey_response_7405e454-508b-4e2a-80a7-e04e84f0bbaa": "Newsletter",
        "$survey_response_a9b47320-a169-4f93-90e6-15375fed4e8d": [
          "It's open source",
          "No meeting bot",
        ],
        "$survey_response_ce59d931-ed0d-4d23-9ab5-55656a1e638a": "Attorney",
        "$survey_response_d58b62d0-c689-4c72-877d-fa949b30ca47": [
          "Voice memos",
        ],
      });
    });
  });
});

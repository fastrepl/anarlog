import type { JSONContent } from "~/editor/session";

export const WELCOME_DISMISSED_KEY = "daily-notes-welcome-dismissed";
export const WELCOME_DATE_KEY = "daily-notes-welcome-date";

export const welcomeContent: JSONContent = {
  type: "doc",
  content: [
    {
      type: "heading",
      attrs: { level: 2 },
      content: [{ type: "text", text: "Welcome to Char!" }],
    },
    {
      type: "paragraph",
      content: [
        {
          type: "text",
          text: "Every day you get a fresh note for your task, ideas and meetings \u2014 write it any way you want and build your own workflow.",
        },
      ],
    },
    {
      type: "paragraph",
      content: [
        {
          type: "text",
          text: "Char record your daily work on screen, learn of your day and projects to help you with your routine. All data stays and processed locally and never leave your computer.",
        },
      ],
    },
    {
      type: "paragraph",
      content: [
        {
          type: "text",
          marks: [
            {
              type: "link",
              attrs: { href: "char://settings/app" },
            },
          ],
          text: "Setup permissions",
        },
        {
          type: "text",
          text: " and read more in ",
        },
        {
          type: "text",
          marks: [
            {
              type: "link",
              attrs: { href: "https://char.com/docs" },
            },
          ],
          text: "Docs",
        },
        {
          type: "text",
          text: " about how Char record your day",
        },
      ],
    },
    {
      type: "heading",
      attrs: { level: 3 },
      content: [
        { type: "text", text: "This is what you can do with Daily Notes" },
      ],
    },
    {
      type: "taskList",
      content: [
        {
          type: "taskItem",
          attrs: { checked: false },
          content: [
            {
              type: "paragraph",
              content: [
                {
                  type: "text",
                  text: "You can write and create any lists there. Just type ",
                },
                { type: "text", marks: [{ type: "code" }], text: "/" },
                { type: "text", text: " or in markdown" },
              ],
            },
          ],
        },
        {
          type: "taskItem",
          attrs: { checked: false },
          content: [
            {
              type: "paragraph",
              content: [
                {
                  type: "text",
                  marks: [
                    {
                      type: "link",
                      attrs: { href: "char://settings/intelligence" },
                    },
                  ],
                  text: "Configure",
                },
                {
                  type: "text",
                  text: " your AI to work with there. Char works with Cloud, API providers and local models to transcribe meetings, answer questions and complete your tasks",
                },
              ],
            },
          ],
        },
        {
          type: "taskItem",
          attrs: { checked: false },
          content: [
            {
              type: "paragraph",
              content: [
                {
                  type: "text",
                  text: "All your meetings appears there. Click on it to open",
                },
              ],
            },
            {
              type: "session",
              attrs: {
                sessionId: "welcome-demo",
                status: null,
                checked: null,
              },
              content: [
                {
                  type: "paragraph",
                  content: [{ type: "text", text: "This is dummy meeting" }],
                },
              ],
            },
            {
              type: "paragraph",
              content: [
                {
                  type: "text",
                  text: " Char will transcribe them by using all context around: audio, presentations, your notes",
                },
              ],
            },
          ],
        },
        {
          type: "taskItem",
          attrs: { checked: false },
          content: [
            {
              type: "paragraph",
              content: [
                {
                  type: "text",
                  text: "Create a new recording with ",
                },
                { type: "text", marks: [{ type: "code" }], text: "/" },
                { type: "text", text: " command or by pressing " },
                {
                  type: "text",
                  marks: [{ type: "code" }],
                  text: "new recording",
                },
                {
                  type: "text",
                  text: ". It will appear in your daily note",
                },
              ],
            },
          ],
        },
        {
          type: "taskItem",
          attrs: { checked: false },
          content: [
            {
              type: "paragraph",
              content: [
                { type: "text", text: "Explore how to add " },
                {
                  type: "text",
                  marks: [
                    {
                      type: "link",
                      attrs: { href: "char://settings/agent" },
                    },
                  ],
                  text: "integrations",
                },
                { type: "text", text: " in Char to accelerate your work" },
              ],
            },
          ],
        },
        {
          type: "taskItem",
          attrs: { checked: false },
          content: [
            {
              type: "paragraph",
              content: [
                { type: "text", text: "Read " },
                {
                  type: "text",
                  marks: [
                    {
                      type: "link",
                      attrs: { href: "https://char.com/docs" },
                    },
                  ],
                  text: "docs",
                },
                {
                  type: "text",
                  text: " to integrate Char in your workflow.",
                },
              ],
            },
          ],
        },
        {
          type: "taskItem",
          attrs: { checked: false },
          content: [
            {
              type: "paragraph",
              content: [
                { type: "text", text: "Join us on " },
                {
                  type: "text",
                  marks: [
                    {
                      type: "link",
                      attrs: { href: "https://x.com/char" },
                    },
                  ],
                  text: "X",
                },
                { type: "text", text: " or " },
                {
                  type: "text",
                  marks: [
                    {
                      type: "link",
                      attrs: { href: "https://discord.gg/char" },
                    },
                  ],
                  text: "Discord",
                },
                { type: "text", text: " to get latest updates" },
              ],
            },
          ],
        },
      ],
    },
    {
      type: "horizontalRule",
    },
    {
      type: "heading",
      attrs: { level: 3 },
      content: [{ type: "text", text: "Enjoy Char! There is your day:" }],
    },
  ],
};

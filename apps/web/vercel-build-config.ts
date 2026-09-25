import { IMAGE_SIZES } from "./src/lib/image-sizes.ts";

const routes = [
  {
    src: "^/images/blog/.*$",
    headers: {
      "Cache-Control": "public, max-age=31536000, immutable",
      "X-Content-Type-Options": "nosniff",
    },
    continue: true,
  },
  {
    src: "^/apt/dists/.*$",
    headers: {
      "Cache-Control": "public, max-age=300, must-revalidate",
      "X-Content-Type-Options": "nosniff",
    },
    continue: true,
  },
  {
    src: "^/apt/anarlog\\-archive\\-keyring\\.asc$",
    headers: {
      "Cache-Control": "public, max-age=31536000, immutable",
      "X-Content-Type-Options": "nosniff",
    },
    continue: true,
  },
  {
    src: "^/api/assets/blog/(.*)$",
    status: 301,
    headers: {
      Location: "/images/blog/$1",
    },
  },
  {
    src: "^/apt/pool/([^/]+)/amd64/anarlog\\.deb$",
    status: 302,
    headers: {
      Location:
        "https://github.com/fastrepl/anarlog/releases/download/$1/anarlog-linux-x86_64.deb",
    },
  },
  {
    src: "^/apt/pool/([^/]+)/arm64/anarlog\\.deb$",
    status: 302,
    headers: {
      Location:
        "https://github.com/fastrepl/anarlog/releases/download/$1/anarlog-linux-aarch64.deb",
    },
  },
  {
    src: "^/update/(.*)$",
    has: [
      {
        type: "host",
        value: "desktop.anarlog.so",
      },
    ],
    status: 302,
    headers: {
      Location: "https://cdn.crabnebula.app/update/fastrepl/hyprnote2/$1",
    },
  },
  {
    src: "^/download/(.*)$",
    has: [
      {
        type: "host",
        value: "desktop.anarlog.so",
      },
    ],
    status: 302,
    headers: {
      Location: "https://cdn.crabnebula.app/download/fastrepl/hyprnote2/$1",
    },
  },
  {
    src: "^/$",
    has: [
      {
        type: "host",
        value: "desktop.anarlog.so",
      },
    ],
    status: 302,
    headers: {
      Location: "https://anarlog.so/download",
    },
  },
  {
    src: "^/(.*)$",
    has: [
      {
        type: "host",
        value: "desktop2.hyprnote.com",
      },
    ],
    status: 302,
    headers: {
      Location: "https://desktop.anarlog.so/$1",
    },
  },
  {
    src: "^/skill/?$",
    status: 301,
    headers: {
      Location: "/skill.md",
    },
  },
  {
    src: "^/skill\\.md$",
    dest: "https://docs.anarlog.so/skill.md",
    transforms: [
      {
        type: "request.headers",
        op: "set",
        target: {
          key: "Origin",
        },
        args: "docs.anarlog.so",
      },
    ],
  },
  {
    src: "^/skills/?$",
    status: 301,
    headers: {
      Location: "https://docs.anarlog.so/agents/skills",
    },
  },
  {
    src: "^/\\.well-known/skills/(.*)$",
    dest: "https://docs.anarlog.so/.well-known/skills/$1",
    transforms: [
      {
        type: "request.headers",
        op: "set",
        target: {
          key: "Origin",
        },
        args: "docs.anarlog.so",
      },
    ],
  },
  {
    src: "^/\\.well-known/agent-skills/(.*)$",
    dest: "https://docs.anarlog.so/.well-known/agent-skills/$1",
    transforms: [
      {
        type: "request.headers",
        op: "set",
        target: {
          key: "Origin",
        },
        args: "docs.anarlog.so",
      },
    ],
  },
  {
    src: "^/\\.well-known/vercel/(.*)$",
    dest: "https://docs.anarlog.so/.well-known/vercel/$1",
    transforms: [
      {
        type: "request.headers",
        op: "set",
        target: {
          key: "Origin",
        },
        args: "docs.anarlog.so",
      },
    ],
  },
  {
    src: "^/docs/?$",
    status: 301,
    headers: {
      Location: "https://docs.anarlog.so",
    },
  },
  {
    src: "^/docs/(.*)$",
    status: 301,
    headers: {
      Location: "https://docs.anarlog.so/$1",
    },
  },
  {
    src: "^/auth/?$",
    has: [
      {
        type: "host",
        value: "hyprnote.com",
      },
    ],
    status: 301,
    headers: {
      Location: "https://char.com/auth",
    },
  },
  {
    src: "^/callback/(.*)$",
    has: [
      {
        type: "host",
        value: "hyprnote.com",
      },
    ],
    status: 301,
    headers: {
      Location: "https://char.com/callback/$1",
    },
  },
  {
    src: "^/blog/(.*)$",
    has: [
      {
        type: "host",
        value: "hyprnote.com",
      },
    ],
    status: 301,
    headers: {
      Location: "https://char.com/blog/$1",
    },
  },
  {
    src: "^/docs/(.*)$",
    has: [
      {
        type: "host",
        value: "hyprnote.com",
      },
    ],
    status: 301,
    headers: {
      Location: "https://docs.anarlog.so",
    },
  },
  {
    src: "^/changelog/(.*)$",
    has: [
      {
        type: "host",
        value: "hyprnote.com",
      },
    ],
    status: 301,
    headers: {
      Location: "https://char.com/changelog/$1",
    },
  },
  {
    src: "^/gallery/(.*)$",
    has: [
      {
        type: "host",
        value: "hyprnote.com",
      },
    ],
    status: 301,
    headers: {
      Location: "https://char.com/gallery/$1",
    },
  },
  {
    src: "^/integrations/(.*)$",
    has: [
      {
        type: "host",
        value: "hyprnote.com",
      },
    ],
    status: 301,
    headers: {
      Location: "https://char.com/integrations/$1",
    },
  },
  {
    src: "^/legal/(.*)$",
    has: [
      {
        type: "host",
        value: "hyprnote.com",
      },
    ],
    status: 301,
    headers: {
      Location: "https://anarlog.so/$1",
    },
  },
  {
    src: "^/blog/filesystem-is-coretex/?$",
    status: 301,
    headers: {
      Location: "/blog/",
    },
  },
  {
    src: "^/faq/?$",
    status: 301,
    headers: {
      Location: "/",
    },
  },
  {
    src: "^/roadmap/?$",
    status: 301,
    headers: {
      Location: "/changelog/",
    },
  },
  {
    src: "^/roadmap/(.*)$",
    status: 301,
    headers: {
      Location: "/changelog/",
    },
  },
  {
    src: "^/roadmap/?$",
    has: [
      {
        type: "host",
        value: "hyprnote.com",
      },
    ],
    status: 301,
    headers: {
      Location: "https://char.com/changelog/",
    },
  },
  {
    src: "^/roadmap/(.*)$",
    has: [
      {
        type: "host",
        value: "hyprnote.com",
      },
    ],
    status: 301,
    headers: {
      Location: "https://char.com/changelog/",
    },
  },
  {
    src: "^/vs/(.*)$",
    has: [
      {
        type: "host",
        value: "hyprnote.com",
      },
    ],
    status: 301,
    headers: {
      Location: "https://char.com/vs/$1",
    },
  },
  {
    src: "^/product/(.*)$",
    has: [
      {
        type: "host",
        value: "hyprnote.com",
      },
    ],
    status: 301,
    headers: {
      Location: "https://char.com/product/$1",
    },
  },
  {
    src: "^/solution/(.*)$",
    has: [
      {
        type: "host",
        value: "hyprnote.com",
      },
    ],
    status: 301,
    headers: {
      Location: "https://char.com/solution/$1",
    },
  },
  {
    src: "^/company-handbook/(.*)$",
    has: [
      {
        type: "host",
        value: "hyprnote.com",
      },
    ],
    status: 301,
    headers: {
      Location: "https://char.com/company-handbook/$1",
    },
  },
  {
    src: "^/press-kit/(.*)$",
    has: [
      {
        type: "host",
        value: "hyprnote.com",
      },
    ],
    status: 301,
    headers: {
      Location: "https://char.com/press-kit/$1",
    },
  },
  {
    src: "^/download/?$",
    has: [
      {
        type: "host",
        value: "hyprnote.com",
      },
    ],
    status: 301,
    headers: {
      Location: "https://char.com/download",
    },
  },
  {
    src: "^/pricing/?$",
    has: [
      {
        type: "host",
        value: "hyprnote.com",
      },
    ],
    status: 301,
    headers: {
      Location: "https://char.com/pricing",
    },
  },
  {
    src: "^/about/?$",
    has: [
      {
        type: "host",
        value: "hyprnote.com",
      },
    ],
    status: 301,
    headers: {
      Location: "https://char.com/about",
    },
  },
  {
    src: "^/opensource/?$",
    has: [
      {
        type: "host",
        value: "hyprnote.com",
      },
    ],
    status: 301,
    headers: {
      Location: "https://char.com/opensource",
    },
  },
  {
    src: "^/enterprise/?$",
    has: [
      {
        type: "host",
        value: "hyprnote.com",
      },
    ],
    status: 301,
    headers: {
      Location: "https://char.com/enterprise",
    },
  },
  {
    src: "^/security/?$",
    has: [
      {
        type: "host",
        value: "hyprnote.com",
      },
    ],
    status: 301,
    headers: {
      Location: "https://char.com/security",
    },
  },
  {
    src: "^/free/?$",
    has: [
      {
        type: "host",
        value: "hyprnote.com",
      },
    ],
    status: 301,
    headers: {
      Location: "https://char.com/free",
    },
  },
  {
    src: "^/contact/?$",
    has: [
      {
        type: "host",
        value: "hyprnote.com",
      },
    ],
    status: 301,
    headers: {
      Location: "https://char.com/contact",
    },
  },
  {
    src: "^/$",
    has: [
      {
        type: "host",
        value: "hyprnote.com",
      },
    ],
    status: 301,
    headers: {
      Location: "https://char.com",
    },
  },
  {
    src: "^/(.*)$",
    has: [
      {
        type: "host",
        value: "getchar.com",
      },
    ],
    status: 301,
    headers: {
      Location: "https://char.com/$1",
    },
  },
  {
    src: "^/(.*)$",
    has: [
      {
        type: "host",
        value: "www.getchar.com",
      },
    ],
    status: 301,
    headers: {
      Location: "https://char.com/$1",
    },
  },
  {
    src: "^/(.*)$",
    has: [
      {
        type: "host",
        value: "getchar.com",
      },
    ],
    status: 301,
    headers: {
      Location: "https://char.com/$1",
    },
  },
  {
    src: "^/(.*)$",
    has: [
      {
        type: "host",
        value: "www.getchar.com",
      },
    ],
    status: 301,
    headers: {
      Location: "https://char.com/$1",
    },
  },
  {
    src: "^/blog/(.*)$",
    has: [
      {
        type: "host",
        value: "char.com",
      },
    ],
    status: 301,
    headers: {
      Location: "https://anarlog.so/blog/$1",
    },
  },
  {
    src: "^/blog/(.*)$",
    has: [
      {
        type: "host",
        value: "www.char.com",
      },
    ],
    status: 301,
    headers: {
      Location: "https://anarlog.so/blog/$1",
    },
  },
  {
    src: "^/legal/(.*)$",
    status: 301,
    headers: {
      Location: "/$1",
    },
  },
  {
    src: "^/legal/(.*)$",
    has: [
      {
        type: "host",
        value: "char.com",
      },
    ],
    status: 301,
    headers: {
      Location: "https://anarlog.so/$1",
    },
  },
  {
    src: "^/legal/(.*)$",
    has: [
      {
        type: "host",
        value: "www.char.com",
      },
    ],
    status: 301,
    headers: {
      Location: "https://anarlog.so/$1",
    },
  },
  {
    src: "^/privacy/?$",
    has: [
      {
        type: "host",
        value: "char.com",
      },
    ],
    status: 301,
    headers: {
      Location: "https://anarlog.so/privacy",
    },
  },
  {
    src: "^/privacy/?$",
    has: [
      {
        type: "host",
        value: "www.char.com",
      },
    ],
    status: 301,
    headers: {
      Location: "https://anarlog.so/privacy",
    },
  },
  {
    src: "^/terms/?$",
    has: [
      {
        type: "host",
        value: "char.com",
      },
    ],
    status: 301,
    headers: {
      Location: "https://anarlog.so/terms",
    },
  },
  {
    src: "^/terms/?$",
    has: [
      {
        type: "host",
        value: "www.char.com",
      },
    ],
    status: 301,
    headers: {
      Location: "https://anarlog.so/terms",
    },
  },
] as const;

export const vercelBuildConfig = {
  version: 3 as const,
  routes: [...routes],
  images: {
    sizes: [...IMAGE_SIZES],
    domains: ["char.com", "hyprnote.com", "ijoptyyjrfqwaqhyxkxj.supabase.co"],
    formats: ["image/webp" as const],
    minimumCacheTTL: 3600,
  },
  crons: [
    {
      path: "/api/cron/loops-account-onboarding",
      schedule: "*/5 * * * *",
    },
    {
      path: "/api/cron/loops-trial-ending-reminders",
      schedule: "*/5 * * * *",
    },
    {
      path: "/api/cron/posthog-account-events",
      schedule: "*/5 * * * *",
    },
  ],
};

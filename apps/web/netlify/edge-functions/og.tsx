// deno-lint-ignore no-import-prefix no-unused-vars
import React from "https://esm.sh/react@18.2.0";
// deno-lint-ignore no-import-prefix
import { z } from "https://deno.land/x/zod@v3.23.8/mod.ts";

const meetingSchema = z.object({
  type: z.literal("meeting"),
  title: z.string(),
  headers: z.array(z.string()),
});

const templatesSchema = z.object({
  type: z.literal("templates"),
  title: z.string(),
  category: z.string(),
  description: z.string().optional(),
});

const shortcutsSchema = z.object({
  type: z.literal("shortcuts"),
  title: z.string(),
  category: z.string(),
  description: z.string().optional(),
});

const changelogSchema = z.object({
  type: z.literal("changelog"),
  version: z.string(),
});

const blogSchema = z.object({
  type: z.literal("blog"),
  title: z.string(),
  description: z.string().optional(),
  author: z.string(),
  date: z.string(),
});

const docsSchema = z.object({
  type: z.literal("docs"),
  title: z.string(),
  section: z.string(),
  description: z.string().optional(),
});

const handbookSchema = z.object({
  type: z.literal("handbook"),
  title: z.string(),
  section: z.string(),
  description: z.string().optional(),
});

const jobsSchema = z.object({
  type: z.literal("jobs"),
  title: z.string(),
  description: z.string().optional(),
  backgroundImage: z.string(),
});

const OGSchema = z.discriminatedUnion("type", [
  meetingSchema,
  templatesSchema,
  shortcutsSchema,
  changelogSchema,
  blogSchema,
  docsSchema,
  handbookSchema,
  jobsSchema,
]);

function preventWidow(text: string): string {
  const words = text.trim().split(/\s+/);
  if (words.length <= 2) return text;

  const last = words.pop()!;
  const secondLast = words.pop()!;
  const lastChunk = `${secondLast}\u00A0${last}`;

  return [...words, lastChunk].join(" ");
}

function parseSearchParams(url: URL): z.infer<typeof OGSchema> | null {
  const type = url.searchParams.get("type");
  if (!type) {
    return null;
  }

  if (type === "changelog") {
    const version = url.searchParams.get("version");

    const result = OGSchema.safeParse({ type, version });
    return result.success ? result.data : null;
  }

  if (type === "blog") {
    const title = url.searchParams.get("title");
    const description = url.searchParams.get("description") || undefined;
    const author = url.searchParams.get("author") || undefined;
    const date = url.searchParams.get("date") || undefined;

    const result = OGSchema.safeParse({
      type,
      title,
      description,
      author,
      date,
    });
    return result.success ? result.data : null;
  }

  if (type === "docs") {
    const title = url.searchParams.get("title");
    const section = url.searchParams.get("section");
    const description = url.searchParams.get("description") || undefined;

    const result = OGSchema.safeParse({ type, title, section, description });
    return result.success ? result.data : null;
  }

  if (type === "handbook") {
    const title = url.searchParams.get("title");
    const section = url.searchParams.get("section");
    const description = url.searchParams.get("description") || undefined;

    const result = OGSchema.safeParse({ type, title, section, description });
    return result.success ? result.data : null;
  }

  if (type === "jobs") {
    const title = url.searchParams.get("title");
    const description = url.searchParams.get("description") || undefined;
    const backgroundImage = url.searchParams.get("backgroundImage");

    const result = OGSchema.safeParse({
      type,
      title,
      description,
      backgroundImage,
    });
    return result.success ? result.data : null;
  }

  if (type === "templates") {
    const title = url.searchParams.get("title");
    const category = url.searchParams.get("category");
    const description = url.searchParams.get("description") || undefined;

    const result = OGSchema.safeParse({ type, title, category, description });
    return result.success ? result.data : null;
  }

  if (type === "shortcuts") {
    const title = url.searchParams.get("title");
    const category = url.searchParams.get("category");
    const description = url.searchParams.get("description") || undefined;

    const result = OGSchema.safeParse({ type, title, category, description });
    return result.success ? result.data : null;
  }

  const title = url.searchParams.get("title");
  const headers = url.searchParams.getAll("headers");

  const result = OGSchema.safeParse({ type, title, headers });
  return result.success ? result.data : null;
}

function renderMeetingTemplate(params: z.infer<typeof meetingSchema>) {
  return (
    <div
      style={{
        width: "100%",
        height: "100%",
        display: "flex",
        flexDirection: "column",
        background: "linear-gradient(135deg, #667eea 0%, #764ba2 100%)",
        padding: "80px",
      }}
    >
      <div
        style={{
          display: "flex",
          flexDirection: "column",
          background: "white",
          borderRadius: "24px",
          padding: "80px",
          width: "100%",
          height: "100%",
        }}
      >
        <div
          style={{
            fontSize: 56,
            fontWeight: 700,
            color: "#1a202c",
            marginBottom: "40px",
          }}
        >
          {params.title}
        </div>
        <div style={{ display: "flex", flexDirection: "column", gap: "20px" }}>
          {params.headers.map((header: string, i: number) => (
            <div
              key={i}
              style={{
                fontSize: 28,
                color: "#4a5568",
                display: "flex",
                alignItems: "center",
              }}
            >
              <div
                style={{
                  width: "8px",
                  height: "8px",
                  borderRadius: "50%",
                  background: "#667eea",
                  marginRight: "16px",
                }}
              />
              {header}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

function renderChangelogTemplate(params: z.infer<typeof changelogSchema>) {
  const isNightly = params.version.includes("nightly");

  if (isNightly) {
    return (
      <div
        style={{
          width: "100%",
          height: "100%",
          position: "relative",
          background:
            "linear-gradient(180deg, #03BCF1 0%, #127FE5 100%), linear-gradient(0deg, #FAFAF9 0%, #E7E5E4 100%)",
          overflow: "hidden",
          display: "flex",
          flexDirection: "column",
        }}
      >
        <div
          style={{
            left: 56,
            top: 436,
            position: "absolute",
            color: "#FAFAF9",
            fontSize: 60,
            fontFamily: "Lora",
            fontWeight: "700",
            wordWrap: "break-word",
            display: "flex",
          }}
        >
          Changelog
        </div>
        <div
          style={{
            left: 56,
            top: 513,
            position: "absolute",
            color: "#F5F5F4",
            fontSize: 48,
            fontFamily: "IBM Plex Mono",
            fontWeight: "400",
            wordWrap: "break-word",
            display: "flex",
          }}
        >
          v.{params.version}
        </div>
        <div
          style={{
            left: 56.25,
            top: 61.12,
            position: "absolute",
            color: "#F5F5F4",
            fontSize: 40,
            fontFamily: "Lora",
            fontWeight: "400",
            wordWrap: "break-word",
            display: "flex",
          }}
        >
          The AI notepad for private meetings
        </div>
        <div
          style={{
            left: 903,
            top: 55,
            position: "absolute",
            textAlign: "right",
            color: "#FAFAF9",
            fontSize: 50,
            fontFamily: "Lora",
            fontWeight: "700",
            wordWrap: "break-word",
            display: "flex",
          }}
        >
          Char.
        </div>
        <div
          style={{
            width: 140,
            height: 0,
            left: 755,
            top: 87,
            position: "absolute",
            borderTop: "2px solid #F5F5F4",
            display: "flex",
          }}
        ></div>
        <img
          style={{
            width: 462,
            height: 462,
            right: 57,
            bottom: -69,
            position: "absolute",
          }}
          src="https://hyprnote.com/api/images/icons/nightly-icon.png"
        />
      </div>
    );
  }

  return (
    <div
      style={{
        width: "100%",
        height: "100%",
        position: "relative",
        background: "linear-gradient(180deg, #A8A29E 0%, #57534E 100%)",
        overflow: "hidden",
        display: "flex",
        flexDirection: "column",
      }}
    >
      <div
        style={{
          left: 56,
          top: 436,
          position: "absolute",
          color: "#FAFAF9",
          fontSize: 60,
          fontFamily: "Lora",
          fontWeight: "700",
          wordWrap: "break-word",
          display: "flex",
        }}
      >
        Changelog
      </div>
      <div
        style={{
          left: 56,
          top: 513,
          position: "absolute",
          color: "#F5F5F4",
          fontSize: 48,
          fontFamily: "IBM Plex Mono",
          fontWeight: "400",
          wordWrap: "break-word",
          display: "flex",
        }}
      >
        v.{params.version}
      </div>
      <div
        style={{
          left: 56.25,
          top: 61.12,
          position: "absolute",
          color: "#F5F5F4",
          fontSize: 40,
          fontFamily: "Lora",
          fontWeight: "400",
          wordWrap: "break-word",
          display: "flex",
        }}
      >
        The AI notepad for private meetings
      </div>
      <div
        style={{
          left: 903,
          top: 55,
          position: "absolute",
          textAlign: "right",
          color: "#FAFAF9",
          fontSize: 50,
          fontFamily: "Lora",
          fontWeight: "700",
          wordWrap: "break-word",
          display: "flex",
        }}
      >
        Char.
      </div>
      <div
        style={{
          width: 140,
          height: 0,
          left: 755,
          top: 87,
          position: "absolute",
          borderTop: "2px solid #F5F5F4",
          display: "flex",
        }}
      ></div>
      <img
        style={{
          width: 462,
          height: 462,
          right: 57,
          bottom: -69,
          position: "absolute",
        }}
        src="https://hyprnote.com/api/images/icons/stable-icon.png"
      />
    </div>
  );
}

// Keep in sync with apps/web/src/lib/team.ts
const AUTHOR_AVATARS: Record<string, string> = {
  "John Jeong": "https://hyprnote.com/api/images/team/john.png",
  "Yujong Lee": "https://hyprnote.com/api/images/team/yujong.png",
  Harshika: "https://hyprnote.com/api/images/team/harshika.jpeg",
};

function getAuthorAvatar(author: string): string {
  return (
    AUTHOR_AVATARS[author] ||
    "https://hyprnote.com/api/images/icons/stable-icon.png"
  );
}

function renderBlogTemplate(params: z.infer<typeof blogSchema>) {
  const authors = params.author
    .split(",")
    .map((a) => a.trim())
    .filter(Boolean);

  return (
    <div
      style={{
        width: "100%",
        height: "100%",
        padding: 60,
        background: "linear-gradient(0deg, #FAFAF9 0%, #E7E5E4 100%)",
        display: "flex",
        flexDirection: "column",
        justifyContent: "space-between",
      }}
    >
      <div style={{ display: "flex", flexDirection: "column", gap: 24 }}>
        <div
          style={{
            width: "100%",
            color: "black",
            fontSize: 60,
            fontFamily: "Lora",
            fontWeight: "700",
            wordWrap: "break-word",
          }}
        >
          {preventWidow(params.title)}
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 16 }}>
          {authors.map((name, i) => (
            <div
              key={i}
              style={{ display: "flex", alignItems: "center", gap: 8 }}
            >
              <img
                style={{ width: 44, height: 44, borderRadius: 1000 }}
                src={getAuthorAvatar(name)}
              />
              <div
                style={{
                  color: "#292524",
                  fontSize: 28,
                  fontFamily: "Lora",
                  fontWeight: "400",
                  wordWrap: "break-word",
                }}
              >
                {name}
              </div>
            </div>
          ))}
        </div>
        <div
          style={{
            color: "#525252",
            fontSize: 24,
            fontFamily: "Lora",
            fontWeight: "400",
            wordWrap: "break-word",
          }}
        >
          {params.date}
        </div>
      </div>
      <div style={{ display: "flex", flexDirection: "column" }}>
        <div
          style={{
            color: "#525252",
            fontSize: 36,
            fontFamily: "Lora",
            fontWeight: "400",
            wordWrap: "break-word",
          }}
        >
          The AI notepad for private meetings
        </div>
        <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
          <img
            style={{ width: 48, height: 48 }}
            src="https://hyprnote.com/api/images/icons/stable-icon.png"
          />
          <div
            style={{
              color: "#292524",
              fontSize: 48,
              fontFamily: "Lora",
              fontWeight: "700",
              wordWrap: "break-word",
            }}
          >
            Char.
          </div>
        </div>
      </div>
    </div>
  );
}

function renderGenericTemplate({
  headerText,
  category,
  title,
  description,
}: {
  headerText: string;
  category: string;
  title: string;
  description?: string;
}) {
  return (
    <div
      style={{
        width: "100%",
        height: "100%",
        padding: 55,
        background: "linear-gradient(0deg, #FAFAF9 0%, #E7E5E4 100%)",
        overflow: "hidden",
        flexDirection: "column",
        justifyContent: "space-between",
        alignItems: "flex-start",
        display: "flex",
      }}
    >
      <div
        style={{
          justifyContent: "flex-start",
          alignItems: "center",
          gap: 12,
          display: "flex",
        }}
      >
        <img
          style={{ width: 48, height: 48 }}
          src="https://hyprnote.com/api/images/icons/stable-icon.png"
        />
        <div
          style={{
            color: "#292524",
            fontSize: 36,
            fontFamily: "Lora",
            fontWeight: "700",
            wordWrap: "break-word",
          }}
        >
          {headerText}
        </div>
      </div>
      <div
        style={{
          alignSelf: "stretch",
          flexDirection: "column",
          justifyContent: "flex-start",
          alignItems: "flex-start",
          gap: 12,
          display: "flex",
        }}
      >
        <div
          style={{
            color: "#525252",
            fontSize: 32,
            fontFamily: "IBM Plex Mono",
            fontWeight: "500",
            wordWrap: "break-word",
          }}
        >
          {category}
        </div>
        <div
          style={{
            alignSelf: "stretch",
            color: "#292524",
            fontSize: 60,
            fontFamily: "Lora",
            fontWeight: "700",
            wordWrap: "break-word",
          }}
        >
          {preventWidow(title)}
        </div>
        {description && (
          <div
            style={{
              alignSelf: "stretch",
              color: "#525252",
              fontSize: 36,
              fontFamily: "IBM Plex Mono",
              fontWeight: "400",
              wordWrap: "break-word",
            }}
          >
            {description}
          </div>
        )}
      </div>
    </div>
  );
}

function renderDocsTemplate(params: z.infer<typeof docsSchema>) {
  return renderGenericTemplate({
    headerText: "Char / Docs",
    category: params.section,
    title: params.title,
    description: params.description,
  });
}

function renderHandbookTemplate(params: z.infer<typeof handbookSchema>) {
  return renderGenericTemplate({
    headerText: "Char / Company Handbook",
    category: params.section,
    title: params.title,
    description: params.description,
  });
}

function renderTemplatesTemplate(params: z.infer<typeof templatesSchema>) {
  return renderGenericTemplate({
    headerText: "Char / Meeting Templates",
    category: params.category,
    title: params.title,
    description: params.description,
  });
}

function renderShortcutsTemplate(params: z.infer<typeof shortcutsSchema>) {
  return renderGenericTemplate({
    headerText: "Char / Shortcuts",
    category: params.category,
    title: params.title,
    description: params.description,
  });
}

function renderJobsTemplate(params: z.infer<typeof jobsSchema>) {
  const backgroundUrl = params.backgroundImage.startsWith("/")
    ? `https://hyprnote.com${params.backgroundImage}`
    : params.backgroundImage;

  return (
    <div
      style={{
        width: "100%",
        height: "100%",
        position: "relative",
        display: "flex",
      }}
    >
      <img
        src={backgroundUrl}
        style={{
          position: "absolute",
          width: "100%",
          height: "100%",
          objectFit: "cover",
        }}
      />
      <div
        style={{
          position: "absolute",
          width: "100%",
          height: "100%",
          background: "rgba(0, 0, 0, 0.4)",
        }}
      />
      <div
        style={{
          width: "100%",
          height: "100%",
          padding: 55,
          position: "relative",
          flexDirection: "column",
          justifyContent: "space-between",
          alignItems: "flex-start",
          display: "flex",
        }}
      >
        <div
          style={{
            justifyContent: "flex-start",
            alignItems: "center",
            gap: 12,
            display: "flex",
          }}
        >
          <img
            style={{ width: 48, height: 48 }}
            src="https://hyprnote.com/api/images/icons/stable-icon.png"
          />
          <div
            style={{
              color: "#FAFAF9",
              fontSize: 36,
              fontFamily: "Lora",
              fontWeight: "700",
              wordWrap: "break-word",
            }}
          >
            We're Hiring
          </div>
        </div>
        <div
          style={{
            alignSelf: "stretch",
            flexDirection: "column",
            justifyContent: "flex-start",
            alignItems: "flex-start",
            gap: 16,
            display: "flex",
          }}
        >
          <div
            style={{
              alignSelf: "stretch",
              color: "#FAFAF9",
              fontSize: 72,
              fontFamily: "Lora",
              fontWeight: "700",
              wordWrap: "break-word",
            }}
          >
            {preventWidow(params.title)}
          </div>
          {params.description && (
            <div
              style={{
                alignSelf: "stretch",
                color: "#E7E5E4",
                fontSize: 32,
                fontFamily: "Lora",
                fontWeight: "400",
                wordWrap: "break-word",
              }}
            >
              {params.description}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

export default async function handler(req: Request) {
  const url = new URL(req.url);

  const params = parseSearchParams(url);

  if (!params) {
    return new Response(JSON.stringify({ error: "invalid_parameters" }), {
      status: 400,
      headers: { "Content-Type": "application/json" },
    });
  }

  try {
    // deno-lint-ignore no-import-prefix
    const { ImageResponse } =
      await import("https://deno.land/x/og_edge@0.0.6/mod.ts");

    // https://unpic.pics/og-edge
    let response;
    if (params.type === "changelog") {
      response = renderChangelogTemplate(params);
    } else if (params.type === "blog") {
      response = renderBlogTemplate(params);
    } else if (params.type === "docs") {
      response = renderDocsTemplate(params);
    } else if (params.type === "handbook") {
      response = renderHandbookTemplate(params);
    } else if (params.type === "templates") {
      response = renderTemplatesTemplate(params);
    } else if (params.type === "shortcuts") {
      response = renderShortcutsTemplate(params);
    } else if (params.type === "jobs") {
      response = renderJobsTemplate(params);
    } else {
      response = renderMeetingTemplate(params);
    }

    const needsCustomFonts =
      params.type === "changelog" ||
      params.type === "blog" ||
      params.type === "docs" ||
      params.type === "handbook" ||
      params.type === "templates" ||
      params.type === "shortcuts" ||
      params.type === "jobs";
    const fonts = needsCustomFonts
      ? [
        {
          name: "Lora",
          data: await fetch(
            "https://fonts.gstatic.com/s/lora/v37/0QI6MX1D_JOuGQbT0gvTJPa787z5vCJG.ttf",
          ).then((res) => res.arrayBuffer()),
          weight: 700 as const,
          style: "normal" as const,
        },
        {
          name: "Lora",
          data: await fetch(
            "https://fonts.gstatic.com/s/lora/v37/0QI6MX1D_JOuGQbT0gvTJPa787weuyJGmKxemMeZ.ttf",
          ).then((res) => res.arrayBuffer()),
          weight: 400 as const,
          style: "normal" as const,
        },
        {
          name: "IBM Plex Mono",
          data: await fetch(
            "https://fonts.gstatic.com/s/ibmplexmono/v20/-F63fjptAgt5VM-kVkqdyU8n5ig.ttf",
          ).then((res) => res.arrayBuffer()),
          weight: 400 as const,
          style: "normal" as const,
        },
      ]
      : undefined;

    const imageResponse = new ImageResponse(response, { fonts });
    imageResponse.headers.set(
      "Netlify-CDN-Cache-Control",
      "public, s-maxage=31536000",
    );
    imageResponse.headers.set(
      "Cache-Control",
      "public, max-age=31536000, immutable",
    );
    imageResponse.headers.set("Netlify-Vary", "query");
    return imageResponse;
  } catch (error) {
    console.error("OG image generation failed:", error);
    return new Response(JSON.stringify({ error: "image_generation_failed" }), {
      status: 500,
      headers: { "Content-Type": "application/json" },
    });
  }
}

// https://docs.netlify.com/build/edge-functions/declarations/#declare-edge-functions-inline
export const config = {
  path: "/og",
  cache: "manual",
};

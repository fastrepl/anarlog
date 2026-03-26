import postgres from "postgres";

import { env, requireEnv } from "@/env";

function getSql() {
  return postgres(requireEnv(env.DATABASE_URL, "DATABASE_URL"), {
    prepare: false,
  });
}

function getGitHubHeaders(accept?: string): Record<string, string> {
  const headers: Record<string, string> = {
    "User-Agent": "hyprnote-admin",
    Accept: accept || "application/vnd.github.v3+json",
  };
  if (env.GITHUB_TOKEN) {
    headers.Authorization = `Bearer ${env.GITHUB_TOKEN}`;
  }
  return headers;
}

export interface StarLead {
  id: number;
  github_username: string;
  github_id: number | null;
  avatar_url: string | null;
  profile_url: string | null;
  bio: string | null;
  event_type: string;
  repo_name: string;
  name: string | null;
  company: string | null;
  is_match: boolean | null;
  score: number | null;
  reasoning: string | null;
  researched_at: string | null;
  event_at: string;
  created_at: string;
}

export async function listStarLeads(options?: {
  limit?: number;
  offset?: number;
  researchedOnly?: boolean;
}): Promise<{ leads: StarLead[]; total: number }> {
  const sql = getSql();
  const limit = options?.limit ?? 50;
  const offset = options?.offset ?? 0;

  const countResult = options?.researchedOnly
    ? await sql`SELECT COUNT(*) as count FROM public.github_star_leads WHERE researched_at IS NOT NULL`
    : await sql`SELECT COUNT(*) as count FROM public.github_star_leads`;
  const total = parseInt(String(countResult[0].count), 10);

  const rows = options?.researchedOnly
    ? await sql`SELECT * FROM public.github_star_leads WHERE researched_at IS NOT NULL ORDER BY COALESCE(score, -1) DESC, created_at DESC LIMIT ${limit} OFFSET ${offset}`
    : await sql`SELECT * FROM public.github_star_leads ORDER BY COALESCE(score, -1) DESC, created_at DESC LIMIT ${limit} OFFSET ${offset}`;

  return { leads: rows as unknown as StarLead[], total };
}

interface GitHubUser {
  login: string;
  id: number;
  avatar_url: string;
  html_url: string;
  type: string;
}

interface GitHubEvent {
  type: string;
  actor: {
    login: string;
    id: number;
    avatar_url: string;
    url: string;
  };
  repo: {
    name: string;
  };
  created_at: string;
}

export async function fetchGitHubStargazers(): Promise<{
  added: number;
  total: number;
}> {
  const sql = getSql();
  let added = 0;
  let page = 1;
  const perPage = 100;

  while (true) {
    const response = await fetch(
      `https://api.github.com/repos/fastrepl/char/stargazers?per_page=${perPage}&page=${page}`,
      {
        headers: getGitHubHeaders("application/vnd.github.star+json"),
      },
    );

    if (!response.ok) break;

    const stargazers: Array<{ starred_at: string; user: GitHubUser }> =
      await response.json();
    if (stargazers.length === 0) break;

    for (const s of stargazers) {
      if (s.user.type === "Bot") continue;

      const result = await sql`
        INSERT INTO public.github_star_leads (github_username, github_id, avatar_url, profile_url, event_type, repo_name, event_at)
        VALUES (${s.user.login}, ${s.user.id}, ${s.user.avatar_url}, ${s.user.html_url}, 'star', 'fastrepl/char', ${s.starred_at})
        ON CONFLICT (github_username) DO UPDATE SET
          avatar_url = EXCLUDED.avatar_url,
          github_id = EXCLUDED.github_id
        RETURNING id`;

      if (result.length > 0) {
        added++;
      }
    }

    if (stargazers.length < perPage) break;
    page++;
  }

  const countResult =
    await sql`SELECT COUNT(*) as count FROM public.github_star_leads`;

  return { added, total: parseInt(String(countResult[0].count), 10) };
}

export async function fetchGitHubActivity(): Promise<{
  added: number;
  total: number;
}> {
  const sql = getSql();
  let added = 0;

  const response = await fetch(
    "https://api.github.com/orgs/fastrepl/events?per_page=100",
    {
      headers: getGitHubHeaders(),
    },
  );

  if (!response.ok) {
    return { added: 0, total: 0 };
  }

  const events: GitHubEvent[] = await response.json();

  const eventTypeMap: Record<string, string> = {
    WatchEvent: "star",
    ForkEvent: "fork",
    IssuesEvent: "issue",
    PullRequestEvent: "pr",
    IssueCommentEvent: "comment",
    PushEvent: "push",
    CreateEvent: "create",
  };

  for (const event of events) {
    const eventType = eventTypeMap[event.type] || event.type;
    if (!event.actor.login) continue;

    const userResponse = await fetch(
      `https://api.github.com/users/${event.actor.login}`,
      {
        headers: getGitHubHeaders(),
      },
    );

    let bio: string | null = null;
    if (userResponse.ok) {
      const userData = await userResponse.json();
      bio = userData.bio;
    }

    const profileUrl = `https://github.com/${event.actor.login}`;

    await sql`
      INSERT INTO public.github_star_leads (github_username, github_id, avatar_url, profile_url, bio, event_type, repo_name, event_at)
      VALUES (${event.actor.login}, ${event.actor.id}, ${event.actor.avatar_url}, ${profileUrl}, ${bio}, ${eventType}, ${event.repo.name}, ${event.created_at})
      ON CONFLICT (github_username) DO UPDATE SET
        avatar_url = EXCLUDED.avatar_url,
        bio = COALESCE(EXCLUDED.bio, github_star_leads.bio),
        event_type = EXCLUDED.event_type,
        event_at = GREATEST(EXCLUDED.event_at, github_star_leads.event_at)`;
    added++;
  }

  const countResult =
    await sql`SELECT COUNT(*) as count FROM public.github_star_leads`;

  return { added, total: parseInt(String(countResult[0].count), 10) };
}

const RESEARCH_PROMPT = `You are an assistant to the founders of Char.

Char is a privacy-first AI notepad for meetings — it runs transcription and summarization locally on-device, without bots or cloud recording. Think of it as the "anti-Otter.ai" for professionals who care about privacy.

I'm sending you data about a GitHub user who interacted with our repository (starred, forked, opened an issue, etc). Your job is to exhaustively research this person using the information provided to determine if they are:

1. A potential customer (someone who would benefit from Char)
2. A potential hire (talented developer who could contribute to Char)
3. A potential community contributor

Char's ideal customer profile:
1. Professional who has frequent meetings (sales, consulting, recruiting, healthcare, legal, journalism, engineering management)
2. Privacy-conscious — works with sensitive data
3. Tech-savvy enough to appreciate local AI / on-device processing
4. Uses a Mac (our primary platform)

Char's ideal hire profile:
1. Strong Rust and/or TypeScript developer
2. Experience with audio processing, ML/AI, or desktop apps (Tauri/Electron)
3. Open source contributor
4. Passionate about privacy and local-first software

Return your final response in JSON only with the following schema:
{
  "name": string,
  "company": string,
  "match": boolean,
  "score": number,
  "reasoning": string
}

- The score field is from 0 to 100.
- The company is where they currently work, or "" if unknown.
- For the "reasoning" field, write in Markdown. Include newlines where appropriate.
- If the person works at Char (fastrepl), there is no match and the score is 0.
- Focus on whether they'd be a good customer, hire, or contributor.`;

export async function researchLead(
  username: string,
  openrouterApiKey: string,
): Promise<{
  success: boolean;
  lead?: StarLead;
  error?: string;
}> {
  const sql = getSql();

  const existing =
    await sql`SELECT * FROM public.github_star_leads WHERE github_username = ${username}`;

  if (existing.length === 0) {
    return { success: false, error: "User not found in leads table" };
  }

  const lead = existing[0] as unknown as StarLead;

  const profileResponse = await fetch(
    `https://api.github.com/users/${username}`,
    {
      headers: getGitHubHeaders(),
    },
  );

  let profileData: Record<string, string | number | null> = {};
  if (profileResponse.ok) {
    profileData = await profileResponse.json();
  }

  const reposResponse = await fetch(
    `https://api.github.com/users/${username}/repos?sort=stars&per_page=10`,
    {
      headers: getGitHubHeaders(),
    },
  );

  let topRepos: Array<{
    name: string;
    description: string | null;
    language: string | null;
    stargazers_count: number;
  }> = [];
  if (reposResponse.ok) {
    topRepos = await reposResponse.json();
  }

  const userInfo = `GitHub Username: ${username}
Name: ${profileData.name || "Unknown"}
Bio: ${profileData.bio || "N/A"}
Company: ${profileData.company || "N/A"}
Location: ${profileData.location || "N/A"}
Blog/Website: ${profileData.blog || "N/A"}
Twitter: ${profileData.twitter_username || "N/A"}
Public Repos: ${profileData.public_repos || 0}
Followers: ${profileData.followers || 0}
Following: ${profileData.following || 0}
Profile URL: https://github.com/${username}
Event Type: ${lead.event_type} on ${lead.repo_name}

Top Repositories:
${topRepos
  .slice(0, 5)
  .map(
    (r) =>
      `- ${r.name}: ${r.description || "No description"} (${r.language || "Unknown"}, ${r.stargazers_count} stars)`,
  )
  .join("\n")}`;

  const llmResponse = await fetch(
    "https://openrouter.ai/api/v1/chat/completions",
    {
      method: "POST",
      headers: {
        Authorization: `Bearer ${openrouterApiKey}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        model: "openai/gpt-4o-mini",
        messages: [
          { role: "system", content: RESEARCH_PROMPT },
          {
            role: "user",
            content: `Research this GitHub user:\n\n${userInfo}`,
          },
        ],
        temperature: 0.3,
        response_format: { type: "json_object" },
      }),
    },
  );

  if (!llmResponse.ok) {
    const errText = await llmResponse.text();
    return { success: false, error: `OpenRouter API error: ${errText}` };
  }

  const llmData = await llmResponse.json();
  const content = llmData.choices?.[0]?.message?.content;

  if (!content) {
    return { success: false, error: "No response from LLM" };
  }

  let parsed: {
    name: string;
    company: string;
    match: boolean;
    score: number;
    reasoning: string;
  };
  try {
    parsed = JSON.parse(content);
  } catch {
    return {
      success: false,
      error: `Failed to parse LLM response: ${content}`,
    };
  }

  const parsedName = parsed.name || "";
  const parsedCompany = parsed.company || "";
  const parsedReasoning = parsed.reasoning || "";
  const parsedBio = profileData.bio ? String(profileData.bio) : null;

  await sql`
    UPDATE public.github_star_leads SET
      name = ${parsedName},
      company = ${parsedCompany},
      is_match = ${parsed.match},
      score = ${parsed.score},
      reasoning = ${parsedReasoning},
      bio = COALESCE(bio, ${parsedBio}),
      researched_at = NOW()
    WHERE github_username = ${username}`;

  const updated =
    await sql`SELECT * FROM public.github_star_leads WHERE github_username = ${username}`;

  return { success: true, lead: updated[0] as unknown as StarLead };
}

import postgres from "postgres";

import { env, requireEnv } from "@/env";

const FASTREPL_ORG = "fastrepl";
const CHAR_REPO = "fastrepl/char";
const OPENROUTER_MODEL = "openai/gpt-4o-mini";

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

function getProfileUrl(username: string, profileUrl?: string | null) {
  return profileUrl || `https://github.com/${username}`;
}

function formatEventLabel(eventType: string, count: number) {
  return `${count} ${eventType}${count === 1 ? "" : "s"}`;
}

function buildDigestSummary({
  counts,
  topLeads,
  eventBreakdown,
}: {
  counts: StarLeadDigest["counts"];
  topLeads: StarLead[];
  eventBreakdown: StarLeadDigest["eventBreakdown"];
}) {
  const lines = [
    `${counts.totalLeads} total leads, ${counts.researchedLeads} researched, ${counts.matchedLeads} matched.`,
    counts.needsResearch > 0
      ? `${counts.needsResearch} leads need fresh research.`
      : "All tracked leads are up to date.",
  ];

  if (counts.activeLast7Days > 0) {
    lines.push(
      `${counts.activeLast7Days} leads were active in the last 7 days.`,
    );
  }

  if (eventBreakdown.length > 0) {
    lines.push(
      `Recent activity: ${eventBreakdown
        .map((event) => formatEventLabel(event.eventType, event.count))
        .join(", ")}.`,
    );
  }

  if (topLeads.length > 0) {
    lines.push("Top leads:");
    topLeads.slice(0, 3).forEach((lead, index) => {
      lines.push(
        `${index + 1}. ${lead.name || lead.github_username} (${lead.score ?? 0}/100)${lead.company ? ` - ${lead.company}` : ""}`,
      );
    });
  }

  return lines.join("\n");
}

async function recordLeadEvent(
  sql: ReturnType<typeof postgres>,
  event: {
    githubUsername: string;
    githubId: number | null;
    avatarUrl: string | null;
    profileUrl?: string | null;
    bio?: string | null;
    eventType: string;
    repoName: string;
    eventAt: string;
    eventSource: "stargazers" | "activity";
  },
) {
  const profileUrl = getProfileUrl(event.githubUsername, event.profileUrl);

  const insertedEvent = await sql`
    INSERT INTO public.github_star_lead_events (
      github_username,
      github_id,
      avatar_url,
      profile_url,
      event_type,
      repo_name,
      event_source,
      event_at
    )
    VALUES (
      ${event.githubUsername},
      ${event.githubId},
      ${event.avatarUrl},
      ${profileUrl},
      ${event.eventType},
      ${event.repoName},
      ${event.eventSource},
      ${event.eventAt}
    )
    ON CONFLICT (github_username, event_type, repo_name, event_at) DO NOTHING
    RETURNING id`;

  const leadResult = (await sql`
    INSERT INTO public.github_star_leads (
      github_username,
      github_id,
      avatar_url,
      profile_url,
      bio,
      event_type,
      repo_name,
      event_at
    )
    VALUES (
      ${event.githubUsername},
      ${event.githubId},
      ${event.avatarUrl},
      ${profileUrl},
      ${event.bio || null},
      ${event.eventType},
      ${event.repoName},
      ${event.eventAt}
    )
    ON CONFLICT (github_username) DO UPDATE SET
      github_id = COALESCE(EXCLUDED.github_id, github_star_leads.github_id),
      avatar_url = COALESCE(EXCLUDED.avatar_url, github_star_leads.avatar_url),
      profile_url = COALESCE(EXCLUDED.profile_url, github_star_leads.profile_url),
      bio = COALESCE(EXCLUDED.bio, github_star_leads.bio),
      event_type = CASE
        WHEN EXCLUDED.event_at >= github_star_leads.event_at THEN EXCLUDED.event_type
        ELSE github_star_leads.event_type
      END,
      repo_name = CASE
        WHEN EXCLUDED.event_at >= github_star_leads.event_at THEN EXCLUDED.repo_name
        ELSE github_star_leads.repo_name
      END,
      event_at = GREATEST(EXCLUDED.event_at, github_star_leads.event_at)
    RETURNING (xmax = 0) AS inserted`) as unknown as Array<{
    inserted: boolean;
  }>;

  return {
    newEvent: insertedEvent.length > 0,
    newLead: Boolean(leadResult[0]?.inserted),
  };
}

async function getGitHubProfile(username: string) {
  const profileResponse = await fetch(
    `https://api.github.com/users/${username}`,
    {
      headers: getGitHubHeaders(),
    },
  );

  if (!profileResponse.ok) {
    return {} as Record<string, string | number | null>;
  }

  return (await profileResponse.json()) as Record<
    string,
    string | number | null
  >;
}

async function getTopGitHubRepos(username: string) {
  const reposResponse = await fetch(
    `https://api.github.com/users/${username}/repos?sort=stars&per_page=10`,
    {
      headers: getGitHubHeaders(),
    },
  );

  if (!reposResponse.ok) {
    return [] as Array<{
      name: string;
      description: string | null;
      language: string | null;
      stargazers_count: number;
      fork: boolean;
    }>;
  }

  return (await reposResponse.json()) as Array<{
    name: string;
    description: string | null;
    language: string | null;
    stargazers_count: number;
    fork: boolean;
  }>;
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

export interface StarLeadDigest {
  generatedAt: string;
  counts: {
    totalLeads: number;
    researchedLeads: number;
    matchedLeads: number;
    needsResearch: number;
    activeLast7Days: number;
  };
  eventBreakdown: Array<{
    eventType: string;
    count: number;
  }>;
  topLeads: StarLead[];
  summary: string;
}

export interface StarLeadSyncResult {
  source: "stargazers" | "activity";
  added: number;
  newLeads: number;
  total: number;
}

export interface StarLeadResearchBatchResult {
  attempted: number;
  completed: number;
  failed: Array<{
    username: string;
    error: string;
  }>;
  leads: StarLead[];
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
    ? await sql`SELECT COUNT(*) AS count FROM public.github_star_leads WHERE researched_at IS NOT NULL`
    : await sql`SELECT COUNT(*) AS count FROM public.github_star_leads`;
  const total = parseInt(String(countResult[0].count), 10);

  const rows = options?.researchedOnly
    ? await sql`
        SELECT *
        FROM public.github_star_leads
        WHERE researched_at IS NOT NULL
        ORDER BY COALESCE(score, -1) DESC, event_at DESC
        LIMIT ${limit} OFFSET ${offset}`
    : await sql`
        SELECT *
        FROM public.github_star_leads
        ORDER BY COALESCE(score, -1) DESC, event_at DESC
        LIMIT ${limit} OFFSET ${offset}`;

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

export async function fetchGitHubStargazers(): Promise<StarLeadSyncResult> {
  const sql = getSql();
  let added = 0;
  let newLeads = 0;
  let page = 1;
  const perPage = 100;

  while (true) {
    const response = await fetch(
      `https://api.github.com/repos/${CHAR_REPO}/stargazers?per_page=${perPage}&page=${page}`,
      {
        headers: getGitHubHeaders("application/vnd.github.star+json"),
      },
    );

    if (!response.ok) {
      break;
    }

    const stargazers = (await response.json()) as Array<{
      starred_at: string;
      user: GitHubUser;
    }>;
    if (stargazers.length === 0) {
      break;
    }

    for (const stargazer of stargazers) {
      if (stargazer.user.type === "Bot") {
        continue;
      }

      const result = await recordLeadEvent(sql, {
        githubUsername: stargazer.user.login,
        githubId: stargazer.user.id,
        avatarUrl: stargazer.user.avatar_url,
        profileUrl: stargazer.user.html_url,
        eventType: "star",
        repoName: CHAR_REPO,
        eventAt: stargazer.starred_at,
        eventSource: "stargazers",
      });

      if (result.newEvent) {
        added++;
      }
      if (result.newLead) {
        newLeads++;
      }
    }

    if (stargazers.length < perPage) {
      break;
    }

    page++;
  }

  const countResult =
    await sql`SELECT COUNT(*) AS count FROM public.github_star_leads`;

  return {
    source: "stargazers",
    added,
    newLeads,
    total: parseInt(String(countResult[0].count), 10),
  };
}

export async function fetchGitHubActivity(): Promise<StarLeadSyncResult> {
  const sql = getSql();
  let added = 0;
  let newLeads = 0;
  const profileCache = new Map<
    string,
    Record<string, string | number | null>
  >();

  const response = await fetch(
    `https://api.github.com/orgs/${FASTREPL_ORG}/events?per_page=100`,
    {
      headers: getGitHubHeaders(),
    },
  );

  if (!response.ok) {
    return {
      source: "activity",
      added: 0,
      newLeads: 0,
      total: 0,
    };
  }

  const events = (await response.json()) as GitHubEvent[];
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
    if (!event.actor.login) {
      continue;
    }

    const profileData =
      profileCache.get(event.actor.login) ||
      (await getGitHubProfile(event.actor.login));
    profileCache.set(event.actor.login, profileData);
    if (profileData.type === "Bot") {
      continue;
    }
    const result = await recordLeadEvent(sql, {
      githubUsername: event.actor.login,
      githubId: event.actor.id,
      avatarUrl: event.actor.avatar_url,
      profileUrl: event.actor.url?.replace(
        "api.github.com/users",
        "github.com",
      ),
      bio: profileData.bio ? String(profileData.bio) : null,
      eventType,
      repoName: event.repo.name,
      eventAt: event.created_at,
      eventSource: "activity",
    });

    if (result.newEvent) {
      added++;
    }
    if (result.newLead) {
      newLeads++;
    }
  }

  const countResult =
    await sql`SELECT COUNT(*) AS count FROM public.github_star_leads`;

  return {
    source: "activity",
    added,
    newLeads,
    total: parseInt(String(countResult[0].count), 10),
  };
}

const RESEARCH_PROMPT = `You are an assistant to the founders of Char.

Char is a privacy-first AI notepad for meetings. It records and summarizes conversations locally on-device, without bots or cloud recording. Think of it as the anti-Otter.ai for professionals who care about privacy.

I am sending you structured GitHub lead data for someone who interacted with the Char repository or the surrounding Fastrepl organization. Your job is to qualify whether they are:

1. A likely Char customer
2. A strong potential hire
3. A promising community contributor

Char's ideal customer profile:
1. Professionals with frequent meetings
2. Privacy-conscious teams or individuals
3. Technically opinionated users who value local-first tooling
4. People likely to use a Mac in their workflow

Char's ideal hire profile:
1. Strong Rust and/or TypeScript developers
2. Experience with audio, desktop apps, ML, AI, or developer tooling
3. Meaningful open-source activity
4. Interest in privacy and local-first products

Return JSON only with this schema:
{
  "name": string,
  "company": string,
  "match": boolean,
  "score": number,
  "reasoning": string
}

Rules:
- The score is 0 to 100.
- Company should be "" if unknown.
- The reasoning should be Markdown with concise evidence.
- If the person clearly works at Char or Fastrepl, match is false and score is 0.
- Base your answer only on the supplied data. Do not invent facts.`;

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
  const profileData = await getGitHubProfile(username);
  const topRepos = await getTopGitHubRepos(username);

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
Latest Event: ${lead.event_type} on ${lead.repo_name} at ${lead.event_at}

Top Repositories:
${topRepos
  .slice(0, 5)
  .map(
    (repo) =>
      `- ${repo.name}: ${repo.description || "No description"} (${repo.language || "Unknown"}, ${repo.stargazers_count} stars${repo.fork ? ", fork" : ""})`,
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
        model: OPENROUTER_MODEL,
        messages: [
          { role: "system", content: RESEARCH_PROMPT },
          {
            role: "user",
            content: `Qualify this GitHub lead:\n\n${userInfo}`,
          },
        ],
        temperature: 0.2,
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
      bio = COALESCE(${parsedBio}, bio),
      researched_at = NOW()
    WHERE github_username = ${username}`;

  const updated =
    await sql`SELECT * FROM public.github_star_leads WHERE github_username = ${username}`;

  return { success: true, lead: updated[0] as unknown as StarLead };
}

export async function researchTopLeads(options: {
  limit: number;
  openrouterApiKey: string;
  force?: boolean;
}): Promise<StarLeadResearchBatchResult> {
  const sql = getSql();
  const rows = options.force
    ? await sql`
        SELECT github_username, event_type
        FROM public.github_star_leads
        ORDER BY event_at DESC
        LIMIT ${options.limit}`
    : await sql`
        SELECT github_username, event_type
        FROM public.github_star_leads
        WHERE researched_at IS NULL OR researched_at < event_at
        ORDER BY
          CASE event_type
            WHEN 'pr' THEN 6
            WHEN 'issue' THEN 5
            WHEN 'fork' THEN 4
            WHEN 'star' THEN 3
            WHEN 'comment' THEN 2
            ELSE 1
          END DESC,
          event_at DESC
        LIMIT ${options.limit}`;

  const usernames = (
    rows as unknown as Array<{
      github_username: string;
    }>
  ).map((row) => row.github_username);

  const failed: StarLeadResearchBatchResult["failed"] = [];
  const leads: StarLead[] = [];

  for (const username of usernames) {
    const result = await researchLead(username, options.openrouterApiKey);
    if (result.success && result.lead) {
      leads.push(result.lead);
      continue;
    }

    failed.push({
      username,
      error: result.error || "Research failed",
    });
  }

  return {
    attempted: usernames.length,
    completed: leads.length,
    failed,
    leads,
  };
}

export async function getStarLeadDigest(options?: {
  limit?: number;
}): Promise<StarLeadDigest> {
  const sql = getSql();
  const limit = options?.limit ?? 5;

  const countRows = (await sql`
    SELECT
      COUNT(*)::int AS total_leads,
      COUNT(*) FILTER (WHERE researched_at IS NOT NULL)::int AS researched_leads,
      COUNT(*) FILTER (WHERE is_match IS TRUE)::int AS matched_leads,
      COUNT(*) FILTER (WHERE researched_at IS NULL OR researched_at < event_at)::int AS needs_research,
      COUNT(*) FILTER (WHERE event_at >= NOW() - INTERVAL '7 days')::int AS active_last_7_days
    FROM public.github_star_leads`) as unknown as Array<{
    total_leads: number;
    researched_leads: number;
    matched_leads: number;
    needs_research: number;
    active_last_7_days: number;
  }>;

  const eventBreakdown = (await sql`
    SELECT event_type, COUNT(*)::int AS count
    FROM public.github_star_lead_events
    WHERE event_at >= NOW() - INTERVAL '30 days'
    GROUP BY event_type
    ORDER BY count DESC, event_type ASC
    LIMIT 6`) as unknown as Array<{
    event_type: string;
    count: number;
  }>;

  const topLeads = (await sql`
    SELECT *
    FROM public.github_star_leads
    WHERE researched_at IS NOT NULL
    ORDER BY
      COALESCE(is_match::int, 0) DESC,
      COALESCE(score, -1) DESC,
      event_at DESC
    LIMIT ${limit}`) as unknown as StarLead[];

  const counts = {
    totalLeads: countRows[0]?.total_leads ?? 0,
    researchedLeads: countRows[0]?.researched_leads ?? 0,
    matchedLeads: countRows[0]?.matched_leads ?? 0,
    needsResearch: countRows[0]?.needs_research ?? 0,
    activeLast7Days: countRows[0]?.active_last_7_days ?? 0,
  };

  const normalizedEventBreakdown = eventBreakdown.map((event) => ({
    eventType: event.event_type,
    count: event.count,
  }));

  return {
    generatedAt: new Date().toISOString(),
    counts,
    eventBreakdown: normalizedEventBreakdown,
    topLeads,
    summary: buildDigestSummary({
      counts,
      topLeads,
      eventBreakdown: normalizedEventBreakdown,
    }),
  };
}

export async function runStarLeadPipeline(options?: {
  maxResearch?: number;
  includeStargazers?: boolean;
  includeActivity?: boolean;
  forceResearch?: boolean;
  openrouterApiKey?: string;
}) {
  const syncResults: StarLeadSyncResult[] = [];
  const includeStargazers = options?.includeStargazers ?? true;
  const includeActivity = options?.includeActivity ?? true;
  const maxResearch = options?.maxResearch ?? 10;

  if (includeStargazers) {
    syncResults.push(await fetchGitHubStargazers());
  }

  if (includeActivity) {
    syncResults.push(await fetchGitHubActivity());
  }

  let research: StarLeadResearchBatchResult = {
    attempted: 0,
    completed: 0,
    failed: [],
    leads: [],
  };

  if (maxResearch > 0) {
    const openrouterApiKey =
      options?.openrouterApiKey || process.env.OPENROUTER_API_KEY || "";
    if (!openrouterApiKey) {
      throw new Error(
        "OpenRouter API key is required. Set OPENROUTER_API_KEY or pass openrouterApiKey.",
      );
    }

    research = await researchTopLeads({
      limit: maxResearch,
      openrouterApiKey,
      force: options?.forceResearch,
    });
  }

  const digest = await getStarLeadDigest();

  return {
    sync: syncResults,
    research,
    digest,
  };
}

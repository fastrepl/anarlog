create table if not exists public.github_star_leads (
  id bigint generated always as identity primary key,
  github_username text not null unique,
  github_id bigint,
  avatar_url text,
  profile_url text,
  bio text,
  event_type text not null default 'star',
  repo_name text not null default 'fastrepl/hyprnote',
  name text,
  company text,
  is_match boolean,
  score integer,
  reasoning text,
  researched_at timestamptz,
  event_at timestamptz not null default now(),
  created_at timestamptz not null default now()
);

create index if not exists idx_github_star_leads_score on public.github_star_leads (score desc nulls last);
create index if not exists idx_github_star_leads_event_type on public.github_star_leads (event_type);

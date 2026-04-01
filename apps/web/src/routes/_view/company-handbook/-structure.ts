export const handbookStructure = {
  sections: [
    "about",
    "how-we-work",
    "teams",
    "who-we-want",
    "go-to-market",
    "onboarding",
  ],
  sectionTitles: {
    about: "About",
    "how-we-work": "How We Work",
    teams: "Teams",
    "who-we-want": "Who We Want",
    "go-to-market": "Go To Market",
    onboarding: "Onboarding",
  } as Record<string, string>,
  defaultPages: {
    about: "about/what-char-is",
    "how-we-work": "how-we-work/work-styles",
    teams: "teams/one-team",
    "who-we-want": "who-we-want/core-traits",
    "go-to-market": "go-to-market/customers",
    onboarding: "onboarding/getting-started",
  } as Record<string, string>,
};

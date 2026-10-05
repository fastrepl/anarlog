declare module "virtual:published-changelogs" {
  const entries: Record<
    string,
    {
      raw: string;
      publication:
        | import("../../changelog-publication").ChangelogPublication
        | null;
    }
  >;
  export default entries;
}

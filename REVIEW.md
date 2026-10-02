# Review Guidelines

## Tests

Judge tests by `.agents/skills/testing/SKILL.md`. More tests are not better tests.

- Do not flag a deleted test as lost coverage when the behavior is covered by another test, the code under test was removed, or the test only checked CSS, DOM shape, copy, log text, mock call counts, private state, constants, framework behavior, or file layout.
- Do flag a deleted or missing test for a fixed bug, persistence or migration behavior, sync, auth, billing, permissions, recording and STT lifecycles, CloudSync/E2EE, OpenAPI or binding contracts, release or store policy, or a cancellation, Stop, or recovery race that nothing else exercises. Name the remaining gap, not just the deleted file.
- Flag new tests that fall in the "do not write" list of the testing skill, and new permutations that exercise the same branch as an existing case.
- Flag package-layout or import-boundary checks written as Vitest suites; they belong in oxlint or ESLint rules.

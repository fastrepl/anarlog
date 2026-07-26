const explicitCandidate = process.argv[2];

function emitCandidate(candidate) {
  if (typeof candidate !== "string" || !/^[0-9a-f]{40}$/i.test(candidate)) {
    console.error("The QA candidate must be a full 40-character commit SHA.");
    process.exit(1);
  }
  process.stdout.write(candidate.toLowerCase());
}

if (explicitCandidate) {
  emitCandidate(explicitCandidate);
} else {
  let source = "";
  process.stdin.setEncoding("utf8");
  process.stdin.on("data", (chunk) => {
    source += chunk;
  });
  process.stdin.on("end", () => {
    let status;
    try {
      status = JSON.parse(source);
    } catch {
      console.error(
        "Could not parse `but status --format json`. Set ANARLOG_QA_GIT_SHA to the candidate commit.",
      );
      process.exit(1);
    }

    const stacks = Array.isArray(status.stacks) ? status.stacks : [];
    if (stacks.length !== 1) {
      console.error(
        `Expected exactly one applied GitButler stack, found ${stacks.length}. Set ANARLOG_QA_GIT_SHA to the candidate commit.`,
      );
      process.exit(1);
    }

    const branches = Array.isArray(stacks[0]?.branches)
      ? stacks[0].branches
      : [];
    const candidate = branches[0]?.commits?.[0]?.commitId;
    if (!candidate) {
      console.error(
        "The applied GitButler stack has no unambiguous committed tip. Set ANARLOG_QA_GIT_SHA to the candidate commit.",
      );
      process.exit(1);
    }

    emitCandidate(candidate);
  });
}

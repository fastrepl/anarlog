#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { basename, dirname, join, parse } from "node:path";

const defaults = {
  top: 20,
  minRatio: 1.5,
  minTestLines: 1000,
  minFileRatio: 4,
  minFileLines: 500,
};

const help = `Usage: node .agents/skills/testing/scripts/test-weight.mjs [options]

Reports packages and test files whose test code outweighs their source.

Options:
  --top N              Maximum rows per table (default: ${defaults.top})
  --min-ratio R        Minimum package test/source ratio (default: ${defaults.minRatio})
  --min-test-lines L   Minimum package test lines (default: ${defaults.minTestLines})
  --min-file-ratio F   Minimum test-file/subject ratio (default: ${defaults.minFileRatio})
  --min-file-lines M   Minimum test-file lines (default: ${defaults.minFileLines})
  --json               Print { totals, packages, files } as JSON
  --help               Show this help

Line ratios use non-blank lines. Rust inline tests are split approximately:
lines from the first #[cfg(test)] inline module through EOF count as test lines.
Cases count test declarations: an it.each table counts once.`;

const sourceExtensions = new Set([
  ".ts",
  ".tsx",
  ".mts",
  ".cts",
  ".js",
  ".jsx",
  ".mjs",
  ".cjs",
  ".rs",
]);
const subjectExtensions = [".ts", ".tsx", ".mts", ".js", ".jsx", ".mjs"];
const setOfSegments = (segments) => new Set(segments.split(" "));
const excludedSegments = setOfSegments("node_modules generated dist vendor");
const testSegments = setOfSegments("tests __tests__ e2e __fixtures__ fixtures");
const jsCasePatternStart = String.raw`^\s*(?:it|test|effectIt)(?:\.[A-Za-z]+)*`;
const jsCasePatternEnd = String.raw`(?:\([^)]*\))?\s*\(`;
const jsCasePattern = new RegExp(jsCasePatternStart + jsCasePatternEnd);
const rustCasePattern = /^\s*#\[(?:test|tokio::test|rstest|sqlx::test)\b/;
const rustTestModulePattern = /^\s*#\[cfg\(test\)\]/;
const rustAttributePattern = /^\s*#\[/;
const rustInlineModulePattern = /^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+\w+\s*\{/;
const rustTestFilePattern = /^(?:tests?|.+_tests?)\.rs$/;

function firstRustTestModule(lines) {
  for (let index = 0; index < lines.length; index += 1) {
    if (!rustTestModulePattern.test(lines[index])) continue;
    let next = index + 1;
    while (
      next < lines.length &&
      (lines[next].trim() === "" || rustAttributePattern.test(lines[next]))
    ) {
      next += 1;
    }
    if (next < lines.length && rustInlineModulePattern.test(lines[next])) {
      return index;
    }
  }
  return -1;
}

function parseArguments(argv) {
  const options = { ...defaults, json: false, help: false };
  const numericOptions = new Map([
    ["--top", "top"],
    ["--min-ratio", "minRatio"],
    ["--min-test-lines", "minTestLines"],
    ["--min-file-ratio", "minFileRatio"],
    ["--min-file-lines", "minFileLines"],
  ]);

  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === "--json") {
      options.json = true;
      continue;
    }
    if (argument === "--help") {
      options.help = true;
      continue;
    }

    const separator = argument.indexOf("=");
    const flag = separator === -1 ? argument : argument.slice(0, separator);
    const key = numericOptions.get(flag);
    if (!key) throw new Error(`Unknown option: ${argument}`);

    let value;
    if (separator === -1) value = argv[++index];
    else value = argument.slice(separator + 1);
    const number = Number(value);
    if (value === undefined || !Number.isFinite(number) || number < 0) {
      throw new Error(`Expected a non-negative number for ${flag}`);
    }
    if (key === "top" && !Number.isInteger(number)) {
      throw new Error("--top must be an integer");
    }
    options[key] = number;
  }

  return options;
}

function nonBlankLines(text) {
  return text.split(/\r?\n/).filter((line) => line.trim() !== "");
}

function isIncluded(path) {
  const parts = path.split("/");
  const file = basename(path);
  return (
    sourceExtensions.has(parse(path).ext) &&
    !file.endsWith(".d.ts") &&
    !/\.gen\./.test(file) &&
    !parts.some((part) => excludedSegments.has(part))
  );
}

function isTestFile(path) {
  const parts = path.split("/");
  const file = basename(path);
  return (
    file.includes(".test.") ||
    file.includes(".spec.") ||
    rustTestFilePattern.test(file) ||
    parts.slice(0, -1).some((part) => testSegments.has(part))
  );
}

function readTrackedFile(root, path) {
  return readFileSync(join(root, path), "utf8");
}

function packageFor(path, tracked) {
  let directory = dirname(path);
  while (directory !== ".") {
    if (
      tracked.has(join(directory, "package.json")) ||
      tracked.has(join(directory, "Cargo.toml"))
    ) {
      return directory;
    }
    const parent = dirname(directory);
    if (parent === directory) break;
    directory = parent;
  }
  return "(root)";
}

function countCases(lines, extension) {
  const pattern = extension === ".rs" ? rustCasePattern : jsCasePattern;
  return lines.reduce((count, line) => count + Number(pattern.test(line)), 0);
}

function testSubject(path, trackedSet) {
  const file = basename(path);
  const match = /^(.*)\.(?:test|spec)\.([^.]+)$/.exec(file);
  if (!match) return null;

  const directory = dirname(path);
  const stem = match[1];
  const directories = [directory];
  const parts = directory.split("/");
  const testsIndex = parts.lastIndexOf("tests");
  if (testsIndex !== -1) {
    parts[testsIndex] = "src";
    directories.push(parts.join("/"));
  }
  for (const candidateDirectory of directories) {
    for (const extension of subjectExtensions) {
      const candidate = join(candidateDirectory, `${stem}${extension}`);
      if (trackedSet.has(candidate)) return candidate;
    }
  }
  return null;
}

function sortByTestLines(rows, nameKey) {
  return rows.sort((left, right) => {
    const lineDifference = right.testLines - left.testLines;
    if (lineDifference !== 0) return lineDifference;
    return left[nameKey].localeCompare(right[nameKey]);
  });
}

function formatCells(cells, widths) {
  const paddedCells = cells.map((cell, index) => {
    const value = String(cell);
    return value.padEnd(widths[index]);
  });
  return paddedCells.join("  ");
}

function renderTable(title, headers, rows) {
  console.log(`\n${title}`);
  if (rows.length === 0) {
    console.log("(no rows meet the configured thresholds)");
    return;
  }

  const widths = headers.map((header, index) =>
    Math.max(header.length, ...rows.map((row) => String(row[index]).length)),
  );
  console.log(formatCells(headers, widths));
  console.log(widths.map((width) => "-".repeat(width)).join("  "));
  for (const row of rows) {
    console.log(formatCells(row, widths));
  }
}

function meetsPackageThresholds(row, options) {
  return row.ratio >= options.minRatio && row.testLines >= options.minTestLines;
}

function run(options) {
  const root = execFileSync("git", ["rev-parse", "--show-toplevel"], {
    encoding: "utf8",
  }).trim();
  const trackedFiles = execFileSync("git", ["ls-files", "-z"], {
    cwd: root,
    encoding: "utf8",
  })
    .split("\0")
    .filter(Boolean);
  const trackedSet = new Set(trackedFiles);
  const includedFiles = trackedFiles.filter(isIncluded);
  const packageTotals = new Map();
  const fileRows = [];
  let totalSourceLines = 0;
  let totalTestLines = 0;
  let totalCases = 0;

  for (const path of includedFiles) {
    const lines = readFileSync(join(root, path), "utf8").split(/\r?\n/);
    const extension = parse(path).ext;
    const testFile = isTestFile(path);
    let sourceLines = 0;
    let testLines = 0;
    let testCode = [];

    if (testFile) {
      testCode = lines;
      testLines = nonBlankLines(lines.join("\n")).length;
    } else if (extension === ".rs") {
      const firstTestLine = firstRustTestModule(lines);
      if (firstTestLine === -1) {
        sourceLines = nonBlankLines(lines.join("\n")).length;
      } else {
        const sourceCode = lines.slice(0, firstTestLine).join("\n");
        sourceLines = nonBlankLines(sourceCode).length;
        testCode = lines.slice(firstTestLine);
        testLines = nonBlankLines(testCode.join("\n")).length;
      }
    } else {
      sourceLines = nonBlankLines(lines.join("\n")).length;
    }

    const cases = countCases(testCode, extension);
    const packageName = packageFor(path, trackedSet);
    const packageTotal = packageTotals.get(packageName) ?? {
      package: packageName,
      sourceLines: 0,
      testLines: 0,
      cases: 0,
    };
    packageTotal.sourceLines += sourceLines;
    packageTotal.testLines += testLines;
    packageTotal.cases += cases;
    packageTotals.set(packageName, packageTotal);

    totalSourceLines += sourceLines;
    totalTestLines += testLines;
    totalCases += cases;

    if (testFile && extension !== ".rs") {
      const subject = testSubject(path, trackedSet);
      if (!subject) continue;
      const subjectLines = nonBlankLines(readTrackedFile(root, subject)).length;
      const ratio = testLines / Math.max(subjectLines, 1);
      if (testLines >= options.minFileLines && ratio >= options.minFileRatio) {
        fileRows.push({
          testFile: path,
          subject,
          testLines,
          sourceLines: subjectLines,
          ratio,
          cases,
        });
      }
    }
  }

  const packages = sortByTestLines(
    [...packageTotals.values()]
      .map((row) => ({
        ...row,
        ratio: row.testLines / Math.max(row.sourceLines, 1),
      }))
      .filter((row) => meetsPackageThresholds(row, options)),
    "package",
  ).slice(0, options.top);
  const files = sortByTestLines(fileRows, "testFile").slice(0, options.top);
  const totals = {
    sourceLines: totalSourceLines,
    testLines: totalTestLines,
    cases: totalCases,
  };

  if (options.json) {
    console.log(JSON.stringify({ totals, packages, files }, null, 2));
    return;
  }

  console.log(
    `Totals: ${totalSourceLines} source lines, ${totalTestLines} test lines, ${totalCases} cases`,
  );
  renderTable(
    "Packages",
    ["Package", "Source lines", "Test lines", "Ratio", "Cases"],
    packages.map((row) => [
      row.package,
      row.sourceLines,
      row.testLines,
      row.ratio.toFixed(2),
      row.cases,
    ]),
  );
  renderTable(
    "Test files vs subject",
    ["Test file", "Subject", "Test lines", "Source lines", "Ratio", "Cases"],
    files.map((row) => [
      row.testFile,
      row.subject,
      row.testLines,
      row.sourceLines,
      row.ratio.toFixed(2),
      row.cases,
    ]),
  );
}

try {
  const options = parseArguments(process.argv.slice(2));
  if (options.help) console.log(help);
  else run(options);
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  console.error(help);
  process.exitCode = 1;
}

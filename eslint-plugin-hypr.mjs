const awaitTauriCommands = {
  meta: {
    type: "suggestion",
    docs: {
      description:
        "Enforce await on Tauri IPC commands to prevent race conditions and freezes",
    },
    fixable: "code",
    messages: {
      missingAwait:
        "Tauri command '{{name}}' must be awaited. Unawaited IPC calls can cause race conditions and freezes.",
    },
  },
  createOnce(context) {
    let tauriCommandImports;

    return {
      before() {
        tauriCommandImports = new Set();
      },

      ImportDeclaration(node) {
        const source = node.source.value;

        const isTauriCommands =
          source.startsWith("@hypr/plugin-") || source.endsWith("/tauri.gen");

        if (isTauriCommands) {
          for (const specifier of node.specifiers) {
            if (
              specifier.type === "ImportSpecifier" &&
              specifier.imported.name === "commands"
            ) {
              tauriCommandImports.add(specifier.local.name);
            }
          }
        }
      },

      CallExpression(node) {
        if (
          node.callee.type === "MemberExpression" &&
          node.callee.object.type === "Identifier" &&
          tauriCommandImports.has(node.callee.object.name)
        ) {
          const methodName = node.callee.property.name;
          const current = node.parent;

          if (current.type === "AwaitExpression") {
            return;
          }

          if (
            current.type === "ReturnStatement" ||
            current.type === "ArrowFunctionExpression"
          ) {
            return;
          }

          if (current.type === "ArrayExpression") {
            return;
          }

          context.report({
            node,
            messageId: "missingAwait",
            data: { name: methodName },
            fix(fixer) {
              if (current.type === "ExpressionStatement") {
                const sourceCode =
                  context.sourceCode || context.getSourceCode();
                const callText = sourceCode.getText(node);
                return fixer.replaceText(node, `await ${callText}`);
              }
              return null;
            },
          });
        }
      },
    };
  },
};

const BANNED_TINYBASE_IMPORT_PREFIXES = [
  "tinybase/ui-react",
  "tinybase/synchronizers",
];

const BANNED_TINYBASE_EXACT_IMPORTS = new Set([
  "tinybase/with-schemas",
  "tinybase",
]);

const BANNED_MAIN_STORE_IMPORTS = new Set([
  "~/store/tinybase/store/main",
  "~/store/tinybase/hooks",
]);

const BANNED_MAIN_STORE_IMPORT_PREFIXES = ["~/store/tinybase/hooks/"];

const noRawTinybase = {
  meta: {
    type: "problem",
    docs: {
      description:
        "Ban direct TinyBase UI hook / store access outside of designated hook modules. Consumers must go through domain hooks (~/<domain>/hooks/*) so storage can be swapped in one place.",
    },
    messages: {
      bannedImport:
        "Direct TinyBase import '{{source}}' is not allowed here. Wrap reads/writes in a domain hook module (e.g. ~/<domain>/hooks/) and import from there instead.",
      bannedMainStoreImport:
        "Direct import of '{{source}}' is not allowed in consumer code. Use a domain hook from ~/<domain>/hooks/ instead. If this file is the hook module, add its path to the rule's allowlist in .oxlintrc.json.",
    },
  },
  create(context) {
    return {
      ImportDeclaration(node) {
        if (node.importKind === "type") return;

        const source = node.source.value;
        if (typeof source !== "string") return;

        if (BANNED_TINYBASE_EXACT_IMPORTS.has(source)) {
          context.report({
            node,
            messageId: "bannedImport",
            data: { source },
          });
          return;
        }

        for (const prefix of BANNED_TINYBASE_IMPORT_PREFIXES) {
          if (source === prefix || source.startsWith(prefix + "/")) {
            context.report({
              node,
              messageId: "bannedImport",
              data: { source },
            });
            return;
          }
        }

        const isBannedMainStore =
          BANNED_MAIN_STORE_IMPORTS.has(source) ||
          BANNED_MAIN_STORE_IMPORT_PREFIXES.some((prefix) =>
            source.startsWith(prefix),
          );

        if (isBannedMainStore) {
          const hasRuntimeSpecifier = node.specifiers.some(
            (s) =>
              s.type === "ImportNamespaceSpecifier" ||
              s.type === "ImportDefaultSpecifier" ||
              (s.type === "ImportSpecifier" && s.importKind !== "type"),
          );
          if (!hasRuntimeSpecifier) return;

          context.report({
            node,
            messageId: "bannedMainStoreImport",
            data: { source },
          });
        }
      },
    };
  },
};

const plugin = {
  meta: {
    name: "hypr",
    version: "1.0.0",
  },
  rules: {
    "await-tauri-commands": awaitTauriCommands,
    "no-raw-tinybase": noRawTinybase,
  },
};

export default plugin;

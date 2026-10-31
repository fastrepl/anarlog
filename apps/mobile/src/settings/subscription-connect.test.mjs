import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { registerHooks } from "node:module";
import { test } from "node:test";
import ts from "typescript";

const fixture = (globalThis.subscriptionScreenFixture = {
  released: false,
  cleanup: null,
});
const mocks = {
  "@expo/ui": `export const Column = () => null, Row = Column;
    export function useNativeState() { return { set value(v) { if (globalThis.subscriptionScreenFixture.released) throw new Error('Cannot use shared object that was already released'); } }; }`,
  "@tanstack/react-form": `export function useForm() { return {reset() {}, state: {values: {code: ''}}}; }`,
  "@tanstack/react-query": `export function useMutation() { return {reset() {}}; }
    export function useQuery() { return {}; }
    export function useQueryClient() { return {cancelQueries: async () => {}}; }`,
  "expo-crypto": `export const CryptoDigestAlgorithm = {}, CryptoEncoding = {}, digestStringAsync = () => {}, getRandomBytes = () => {};`,
  "expo-router": `export function useFocusEffect(effect) { globalThis.subscriptionScreenFixture.cleanup = effect(); }`,
  "expo/fetch": `export const fetch = globalThis.fetch;`,
  react: `export const useCallback = fn => fn; export const useRef = current => ({current});`,
  "react/jsx-runtime": `export const Fragment = 'fragment', jsx = () => null, jsxs = jsx;`,
  "react-native": `export const Linking = {};`,
  "./fields": `export const Button = () => null, Text = Button, TextInput = Button;`,
  "./providers": `export const readProviderSetup = () => {}, removeProviderKey = () => {}, saveProviderSetup = () => {};`,
  "./subscription-access": `export const saveSubscriptionCredential = () => {};`,
  "./subscription-oauth": `export const completeClaudeConnect = () => {}, listSubscriptionModels = () => {}, pollSubscriptionConnect = () => {}, startSubscriptionConnect = () => {};`,
  "./theme-provider": `export const useColors = () => ({});`,
};
const component = new URL("./subscription-connect.tsx", import.meta.url);
registerHooks({
  resolve(specifier, context, next) {
    if (mocks[specifier])
      return {
        url: `data:text/javascript,${encodeURIComponent(mocks[specifier])}`,
        shortCircuit: true,
      };
    return next(specifier, context);
  },
  load(url, context, next) {
    if (url === component.href)
      return {
        format: "module",
        shortCircuit: true,
        source: ts.transpileModule(readFileSync(component, "utf8"), {
          compilerOptions: {
            jsx: ts.JsxEmit.ReactJSX,
            module: ts.ModuleKind.ESNext,
            target: ts.ScriptTarget.ES2022,
          },
        }).outputText,
      };
    return next(url, context);
  },
});
const { SubscriptionConnect } = await import(component.href);

test("leaving subscription sign-in after Expo releases its native input does not crash", () => {
  fixture.released = false;
  SubscriptionConnect({
    provider: "claude",
    account: null,
    connected: false,
    onSaved() {},
  });
  fixture.released = true;
  assert.doesNotThrow(() => fixture.cleanup());
});

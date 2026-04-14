import { useEffect, useRef, useState } from "react";
import {
  Platform,
  Pressable,
  ScrollView,
  StyleSheet,
  Text,
  TextInput,
  View,
} from "react-native";
import { SafeAreaView } from "react-native-safe-area-context";

type Row = Record<string, unknown>;
type BridgeModule = typeof import("../db/mobileBridge");
type NativePathsModule = typeof import("../native/mobilePaths");

const DEBUG_USER_ID = "mobile-debug";

function formatError(error: unknown): string {
  if (error instanceof Error) {
    return error.message;
  }

  return String(error);
}

function formatRows(rows: Row[]): string {
  return JSON.stringify(rows, null, 2);
}

function formatJson(value: unknown): string {
  return JSON.stringify(value, null, 2);
}

function Button({
  label,
  onPress,
}: {
  label: string;
  onPress: () => void | Promise<void>;
}) {
  return (
    <Pressable
      accessibilityRole="button"
      onPress={() => {
        void onPress();
      }}
      style={({ pressed }) => [
        styles.button,
        pressed ? styles.buttonPressed : null,
      ]}
    >
      <Text style={styles.buttonLabel}>{label}</Text>
    </Pressable>
  );
}

export default function HomeScreen() {
  const unsubscribeRef = useRef<(() => void) | null>(null);
  const bridgeRef = useRef<Promise<BridgeModule> | null>(null);
  const nativePathsRef = useRef<Promise<NativePathsModule> | null>(null);
  const [dbPath, setDbPath] = useState("");
  const [status, setStatus] = useState("Idle");
  const [rows, setRows] = useState<Row[]>([]);
  const [logs, setLogs] = useState<string[]>([]);
  const [cloudsyncState, setCloudsyncState] = useState("Not loaded");
  const [cloudsyncManagerState, setCloudsyncManagerState] =
    useState("Not configured");
  const [connectionString, setConnectionString] = useState(
    "sqlitecloud://demo.invalid/mobile.db?apikey=demo",
  );
  const [apiKey, setApiKey] = useState("demo-api-key");
  const [token, setToken] = useState("demo-token");
  const [authMode, setAuthMode] = useState<"none" | "apiKey" | "token">(
    "token",
  );
  const [syncTableName, setSyncTableName] = useState("templates");
  const [syncEnabled, setSyncEnabled] = useState(false);
  const [syncIntervalMs, setSyncIntervalMs] = useState("30000");
  const [waitMs, setWaitMs] = useState("1000");
  const [maxRetries, setMaxRetries] = useState("1");

  function appendLog(message: string) {
    const timestamp = new Date().toISOString().slice(11, 19);
    setLogs((current) => [`${timestamp} ${message}`, ...current].slice(0, 18));
  }

  async function getBridge() {
    if (!bridgeRef.current) {
      bridgeRef.current = import("../db/mobileBridge");
    }

    return bridgeRef.current;
  }

  async function getNativePaths() {
    if (!nativePathsRef.current) {
      nativePathsRef.current = import("../native/mobilePaths");
    }

    return nativePathsRef.current;
  }

  async function run(label: string, task: () => Promise<void>) {
    setStatus(`${label}...`);
    try {
      await task();
      setStatus(`${label} complete`);
      appendLog(`${label} complete`);
    } catch (error) {
      const message = formatError(error);
      setStatus(`${label} failed`);
      appendLog(`${label} failed: ${message}`);
    }
  }

  async function handleOpen() {
    await run("Open DB", async () => {
      const [{ getAppDbPath }, bridge] = await Promise.all([
        getNativePaths(),
        getBridge(),
      ]);
      const path = await getAppDbPath();
      setDbPath(path);
      await bridge.execute("SELECT id, date, user_id FROM daily_notes LIMIT 0");
    });
  }

  async function handleSubscribe() {
    await run("Subscribe", async () => {
      const bridge = await getBridge();
      unsubscribeRef.current?.();
      unsubscribeRef.current = await bridge.subscribe<Row>(
        "SELECT id, date, user_id FROM daily_notes WHERE user_id = ? ORDER BY date DESC, id DESC",
        [DEBUG_USER_ID],
        {
          onData: (nextRows) => {
            setRows(nextRows);
            appendLog(`Subscription update: ${nextRows.length} row(s)`);
          },
          onError: (error) => {
            appendLog(`Subscription error: ${error}`);
          },
        },
      );
    });
  }

  async function handleUnsubscribe() {
    await run("Unsubscribe", async () => {
      unsubscribeRef.current?.();
      unsubscribeRef.current = null;
    });
  }

  async function handleInsert() {
    await run("Insert row", async () => {
      const bridge = await getBridge();
      const id = `mobile-${Date.now()}`;
      const date = new Date().toISOString().slice(0, 10);

      await bridge.execute(
        "INSERT INTO daily_notes (id, date, body, user_id) VALUES (?, ?, ?, ?)",
        [id, date, "{}", DEBUG_USER_ID],
      );
    });
  }

  async function handleVersion() {
    await run("CloudSync version", async () => {
      const bridge = await getBridge();
      const version = await bridge.cloudsyncVersion();
      setCloudsyncState(version);
    });
  }

  async function handleInit() {
    await run("CloudSync init", async () => {
      const bridge = await getBridge();
      await bridge.execute(
        "CREATE TABLE IF NOT EXISTS mobile_sync_debug (id TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL DEFAULT '')",
      );
      await bridge.cloudsyncInit("mobile_sync_debug");
    });
  }

  async function handleNetworkInit() {
    await run("CloudSync network init", async () => {
      const bridge = await getBridge();
      await bridge.cloudsyncNetworkInit(connectionString);
    });
  }

  async function handleSetApiKey() {
    await run("CloudSync set API key", async () => {
      const bridge = await getBridge();
      await bridge.cloudsyncNetworkSetApikey(apiKey);
    });
  }

  async function handleSetToken() {
    await run("CloudSync set token", async () => {
      const bridge = await getBridge();
      await bridge.cloudsyncNetworkSetToken(token);
    });
  }

  async function handleSync() {
    await run("CloudSync sync", async () => {
      const bridge = await getBridge();
      const downloadedCount = await bridge.cloudsyncNetworkSync(1_000, 1);
      setCloudsyncState(`Downloaded ${downloadedCount} change(s)`);
    });
  }

  async function handleConfigureCloudsync() {
    await run("Configure CloudSync", async () => {
      const bridge = await getBridge();
      let auth:
        | { type: "none" }
        | { type: "apiKey"; apiKey: string }
        | { type: "token"; token: string };
      if (authMode === "none") {
        auth = { type: "none" };
      } else if (authMode === "apiKey") {
        auth = { type: "apiKey", apiKey };
      } else {
        auth = { type: "token", token };
      }
      await bridge.configureCloudsync({
        connectionString,
        auth,
        tables: [
          {
            tableName: syncTableName,
            enabled: syncEnabled,
          },
        ],
        syncIntervalMs: Number(syncIntervalMs) || 30_000,
        waitMs: Number(waitMs) || undefined,
        maxRetries: Number(maxRetries) || undefined,
      });
      const status = await bridge.cloudsyncStatus();
      setCloudsyncManagerState(formatJson(status));
    });
  }

  async function handleStartCloudsync() {
    await run("Start CloudSync manager", async () => {
      const bridge = await getBridge();
      await bridge.startCloudsync();
      const status = await bridge.cloudsyncStatus();
      setCloudsyncManagerState(formatJson(status));
    });
  }

  async function handleStopCloudsync() {
    await run("Stop CloudSync manager", async () => {
      const bridge = await getBridge();
      await bridge.stopCloudsync();
      const status = await bridge.cloudsyncStatus();
      setCloudsyncManagerState(formatJson(status));
    });
  }

  async function handleManagerStatus() {
    await run("CloudSync manager status", async () => {
      const bridge = await getBridge();
      const status = await bridge.cloudsyncStatus();
      setCloudsyncManagerState(formatJson(status));
    });
  }

  async function handleManagerSyncNow() {
    await run("CloudSync manager sync", async () => {
      const bridge = await getBridge();
      const downloadedCount = await bridge.cloudsyncSyncNow();
      const status = await bridge.cloudsyncStatus();
      setCloudsyncManagerState(formatJson(status));
      appendLog(`Manager sync downloaded ${downloadedCount} change(s)`);
    });
  }

  useEffect(() => {
    if (Platform.OS === "web") {
      setStatus("Unavailable on web");
      return;
    }

    void handleOpen();
    void handleSubscribe();

    return () => {
      unsubscribeRef.current?.();
      unsubscribeRef.current = null;
      void getBridge()
        .then((bridge) => bridge.closeBridge())
        .catch(() => {});
    };
  }, []);

  return (
    <SafeAreaView style={styles.safeArea}>
      <ScrollView contentContainerStyle={styles.content}>
        <View style={styles.header}>
          <Text style={styles.eyebrow}>Expo DB Bring-up</Text>
          <Text style={styles.title}>Hypr Mobile Bridge Debug</Text>
          <Text style={styles.subtitle}>
            Rust owns DB open, execute, subscribe, and CloudSync calls. This
            Expo screen just exercises the bridge.
          </Text>
        </View>

        {Platform.OS === "web" ? (
          <View style={styles.card}>
            <Text style={styles.cardTitle}>Unsupported Platform</Text>
            <Text style={styles.value}>
              The Rust bridge is only wired for iOS and Android development
              builds.
            </Text>
          </View>
        ) : (
          <>
            <View style={styles.card}>
              <Text style={styles.cardTitle}>Runtime</Text>
              <Text style={styles.label}>Status</Text>
              <Text style={styles.value}>{status}</Text>
              <Text style={styles.label}>DB Path</Text>
              <Text style={styles.value}>{dbPath || "Resolving..."}</Text>
              <Text style={styles.label}>CloudSync</Text>
              <Text style={styles.value}>{cloudsyncState}</Text>
            </View>

            <View style={styles.buttonGrid}>
              <Button label="Open DB" onPress={handleOpen} />
              <Button label="Subscribe" onPress={handleSubscribe} />
              <Button label="Unsubscribe" onPress={handleUnsubscribe} />
              <Button label="Insert Row" onPress={handleInsert} />
              <Button label="CloudSync Version" onPress={handleVersion} />
              <Button label="CloudSync Init" onPress={handleInit} />
            </View>

            <View style={styles.card}>
              <Text style={styles.cardTitle}>CloudSync Network</Text>
              <TextInput
                autoCapitalize="none"
                autoCorrect={false}
                onChangeText={setConnectionString}
                placeholder="Connection string"
                placeholderTextColor="#71717a"
                style={styles.input}
                value={connectionString}
              />
              <Button label="Network Init" onPress={handleNetworkInit} />
              <TextInput
                autoCapitalize="none"
                autoCorrect={false}
                onChangeText={setApiKey}
                placeholder="API key"
                placeholderTextColor="#71717a"
                style={styles.input}
                value={apiKey}
              />
              <Button label="Set API Key" onPress={handleSetApiKey} />
              <TextInput
                autoCapitalize="none"
                autoCorrect={false}
                onChangeText={setToken}
                placeholder="Token"
                placeholderTextColor="#71717a"
                style={styles.input}
                value={token}
              />
              <Button label="Set Token" onPress={handleSetToken} />
              <Button label="Sync Now" onPress={handleSync} />
            </View>

            <View style={styles.card}>
              <Text style={styles.cardTitle}>CloudSync Manager</Text>
              <TextInput
                autoCapitalize="none"
                autoCorrect={false}
                onChangeText={setSyncTableName}
                placeholder="Candidate table"
                placeholderTextColor="#71717a"
                style={styles.input}
                value={syncTableName}
              />
              <View style={styles.buttonGrid}>
                <Button
                  label={`Enabled: ${syncEnabled ? "On" : "Off"}`}
                  onPress={() => {
                    setSyncEnabled((current) => !current);
                  }}
                />
                <Button
                  label={`Auth: ${authMode}`}
                  onPress={() => {
                    setAuthMode((current) => {
                      if (current === "token") {
                        return "apiKey";
                      }
                      if (current === "apiKey") {
                        return "none";
                      }
                      return "token";
                    });
                  }}
                />
              </View>
              <TextInput
                autoCapitalize="none"
                autoCorrect={false}
                keyboardType="number-pad"
                onChangeText={setSyncIntervalMs}
                placeholder="Sync interval (ms)"
                placeholderTextColor="#71717a"
                style={styles.input}
                value={syncIntervalMs}
              />
              <TextInput
                autoCapitalize="none"
                autoCorrect={false}
                keyboardType="number-pad"
                onChangeText={setWaitMs}
                placeholder="Wait (ms)"
                placeholderTextColor="#71717a"
                style={styles.input}
                value={waitMs}
              />
              <TextInput
                autoCapitalize="none"
                autoCorrect={false}
                keyboardType="number-pad"
                onChangeText={setMaxRetries}
                placeholder="Max retries"
                placeholderTextColor="#71717a"
                style={styles.input}
                value={maxRetries}
              />
              <View style={styles.buttonGrid}>
                <Button
                  label="Configure Manager"
                  onPress={handleConfigureCloudsync}
                />
                <Button label="Start Manager" onPress={handleStartCloudsync} />
                <Button label="Stop Manager" onPress={handleStopCloudsync} />
                <Button label="Manager Status" onPress={handleManagerStatus} />
                <Button label="Manager Sync" onPress={handleManagerSyncNow} />
              </View>
              <Text style={styles.mono}>{cloudsyncManagerState}</Text>
            </View>

            <View style={styles.card}>
              <Text style={styles.cardTitle}>Rows</Text>
              <Text style={styles.mono}>
                {rows.length ? formatRows(rows) : "[]"}
              </Text>
            </View>

            <View style={styles.card}>
              <Text style={styles.cardTitle}>Logs</Text>
              <Text style={styles.mono}>
                {logs.length ? logs.join("\n") : "No events yet."}
              </Text>
            </View>
          </>
        )}
      </ScrollView>
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  safeArea: {
    flex: 1,
    backgroundColor: "#09090b",
  },
  content: {
    gap: 16,
    padding: 20,
  },
  header: {
    gap: 8,
    paddingBottom: 8,
  },
  eyebrow: {
    color: "#a1a1aa",
    fontSize: 12,
    fontWeight: "600",
    letterSpacing: 1.2,
    textTransform: "uppercase",
  },
  title: {
    color: "#fafafa",
    fontSize: 32,
    fontWeight: "700",
  },
  subtitle: {
    color: "#d4d4d8",
    fontSize: 16,
    lineHeight: 22,
  },
  card: {
    backgroundColor: "#18181b",
    borderColor: "#27272a",
    borderRadius: 16,
    borderWidth: 1,
    gap: 10,
    padding: 16,
  },
  cardTitle: {
    color: "#fafafa",
    fontSize: 18,
    fontWeight: "700",
  },
  label: {
    color: "#a1a1aa",
    fontSize: 12,
    fontWeight: "600",
    textTransform: "uppercase",
  },
  value: {
    color: "#f4f4f5",
    fontSize: 14,
  },
  buttonGrid: {
    flexDirection: "row",
    flexWrap: "wrap",
    gap: 10,
  },
  button: {
    backgroundColor: "#2563eb",
    borderRadius: 999,
    paddingHorizontal: 14,
    paddingVertical: 10,
  },
  buttonPressed: {
    opacity: 0.85,
  },
  buttonLabel: {
    color: "#eff6ff",
    fontSize: 14,
    fontWeight: "600",
  },
  input: {
    backgroundColor: "#09090b",
    borderColor: "#3f3f46",
    borderRadius: 12,
    borderWidth: 1,
    color: "#fafafa",
    paddingHorizontal: 12,
    paddingVertical: 10,
  },
  mono: {
    color: "#e4e4e7",
    fontFamily: Platform.select({ ios: "Menlo", android: "monospace" }),
    fontSize: 12,
    lineHeight: 18,
  },
});

import { describe, expect, test } from "vitest";

import {
  chatJsonToData,
  createEmptyLoadedChatData,
  mergeLoadedData,
} from "./load";
import { tablesToChatJsonList } from "./save";
import type { ChatJson, LoadedChatData } from "./types";

import type { TablesContent } from "~/store/tinybase/persister/shared";

describe("chatJsonToData", () => {
  test("converts ChatJson to LoadedChatData", () => {
    const json: ChatJson = {
      chat_group: {
        id: "group-1",
        user_id: "user-1",
        created_at: "2024-01-01T00:00:00Z",
        title: "Test Chat",
      },
      messages: [
        {
          id: "msg-1",
          user_id: "user-1",
          created_at: "2024-01-01T00:00:01Z",
          chat_group_id: "group-1",
          role: "user",
          content: "Hello",
          metadata: "{}",
          parts: "[]",
        },
        {
          id: "msg-2",
          user_id: "user-1",
          created_at: "2024-01-01T00:00:02Z",
          chat_group_id: "group-1",
          role: "assistant",
          content: "Hi there!",
          metadata: "{}",
          parts: "[]",
        },
      ],
    };

    const data = chatJsonToData(json);

    expect(data.chat_groups["group-1"]).toEqual({
      user_id: "user-1",
      created_at: "2024-01-01T00:00:00Z",
      title: "Test Chat",
    });

    expect(data.chat_messages["msg-1"]).toEqual({
      user_id: "user-1",
      created_at: "2024-01-01T00:00:01Z",
      chat_group_id: "group-1",
      role: "user",
      content: "Hello",
      metadata: "{}",
      parts: "[]",
    });

    expect(data.chat_messages["msg-2"]).toEqual({
      user_id: "user-1",
      created_at: "2024-01-01T00:00:02Z",
      chat_group_id: "group-1",
      role: "assistant",
      content: "Hi there!",
      metadata: "{}",
      parts: "[]",
    });
  });

  test("handles empty messages array", () => {
    const json: ChatJson = {
      chat_group: {
        id: "group-1",
        user_id: "user-1",
        created_at: "2024-01-01T00:00:00Z",
        title: "Empty Chat",
      },
      messages: [],
    };

    const data = chatJsonToData(json);

    expect(data.chat_groups["group-1"]).toBeDefined();
    expect(Object.keys(data.chat_messages)).toHaveLength(0);
  });
});

describe("mergeLoadedData", () => {
  test("merges multiple LoadedChatData into one", () => {
    const data1: LoadedChatData = {
      chat_groups: {
        "group-1": {
          user_id: "user-1",
          created_at: "2024-01-01T00:00:00Z",
          title: "Chat 1",
        },
      },
      chat_messages: {
        "msg-1": {
          user_id: "user-1",
          created_at: "2024-01-01T00:00:01Z",
          chat_group_id: "group-1",
          role: "user",
          content: "Hello",
          metadata: "{}",
          parts: "[]",
        },
      },
    };

    const data2: LoadedChatData = {
      chat_groups: {
        "group-2": {
          user_id: "user-1",
          created_at: "2024-01-02T00:00:00Z",
          title: "Chat 2",
        },
      },
      chat_messages: {
        "msg-2": {
          user_id: "user-1",
          created_at: "2024-01-02T00:00:01Z",
          chat_group_id: "group-2",
          role: "user",
          content: "Hi",
          metadata: "{}",
          parts: "[]",
        },
      },
    };

    const merged = mergeLoadedData([data1, data2]);

    expect(Object.keys(merged.chat_groups)).toHaveLength(2);
    expect(merged.chat_groups["group-1"]).toBeDefined();
    expect(merged.chat_groups["group-2"]).toBeDefined();

    expect(Object.keys(merged.chat_messages)).toHaveLength(2);
    expect(merged.chat_messages["msg-1"]).toBeDefined();
    expect(merged.chat_messages["msg-2"]).toBeDefined();
  });

  test("returns empty data for empty array", () => {
    const merged = mergeLoadedData([]);

    expect(Object.keys(merged.chat_groups)).toHaveLength(0);
    expect(Object.keys(merged.chat_messages)).toHaveLength(0);
  });
});

describe("tablesToChatJsonList", () => {
  test("converts TablesContent to ChatJson array", () => {
    const tables: TablesContent = {
      chat_groups: {
        "group-1": {
          user_id: "user-1",
          created_at: "2024-01-01T00:00:00Z",
          title: "Test Chat",
        },
      },
      chat_messages: {
        "msg-1": {
          user_id: "user-1",
          created_at: "2024-01-01T00:00:01Z",
          chat_group_id: "group-1",
          role: "user",
          content: "Hello",
          metadata: "{}",
          parts: "[]",
        },
        "msg-2": {
          user_id: "user-1",
          created_at: "2024-01-01T00:00:02Z",
          chat_group_id: "group-1",
          role: "assistant",
          content: "Hi!",
          metadata: "{}",
          parts: "[]",
        },
      },
    };

    const result = tablesToChatJsonList(tables);

    expect(result).toHaveLength(1);
    expect(result[0].chat_group.id).toBe("group-1");
    expect(result[0].messages).toHaveLength(2);
    expect(result[0].messages[0].id).toBe("msg-1");
    expect(result[0].messages[1].id).toBe("msg-2");
  });

  test("sorts messages by created_at", () => {
    const tables: TablesContent = {
      chat_groups: {
        "group-1": {
          user_id: "user-1",
          created_at: "2024-01-01T00:00:00Z",
          title: "Test Chat",
        },
      },
      chat_messages: {
        "msg-2": {
          user_id: "user-1",
          created_at: "2024-01-01T00:00:02Z",
          chat_group_id: "group-1",
          role: "assistant",
          content: "Second",
          metadata: "{}",
          parts: "[]",
        },
        "msg-1": {
          user_id: "user-1",
          created_at: "2024-01-01T00:00:01Z",
          chat_group_id: "group-1",
          role: "user",
          content: "First",
          metadata: "{}",
          parts: "[]",
        },
      },
    };

    const result = tablesToChatJsonList(tables);

    expect(result[0].messages[0].content).toBe("First");
    expect(result[0].messages[1].content).toBe("Second");
  });

  test("excludes messages without matching chat group", () => {
    const tables: TablesContent = {
      chat_groups: {
        "group-1": {
          user_id: "user-1",
          created_at: "2024-01-01T00:00:00Z",
          title: "Test Chat",
        },
      },
      chat_messages: {
        "msg-1": {
          user_id: "user-1",
          created_at: "2024-01-01T00:00:01Z",
          chat_group_id: "group-1",
          role: "user",
          content: "Valid",
          metadata: "{}",
          parts: "[]",
        },
        "msg-orphan": {
          user_id: "user-1",
          created_at: "2024-01-01T00:00:02Z",
          chat_group_id: "non-existent-group",
          role: "user",
          content: "Orphan",
          metadata: "{}",
          parts: "[]",
        },
      },
    };

    const result = tablesToChatJsonList(tables);

    expect(result).toHaveLength(1);
    expect(result[0].messages).toHaveLength(1);
    expect(result[0].messages[0].content).toBe("Valid");
  });

  test("handles multiple chat groups", () => {
    const tables: TablesContent = {
      chat_groups: {
        "group-1": {
          user_id: "user-1",
          created_at: "2024-01-01T00:00:00Z",
          title: "Chat 1",
        },
        "group-2": {
          user_id: "user-1",
          created_at: "2024-01-02T00:00:00Z",
          title: "Chat 2",
        },
      },
      chat_messages: {
        "msg-1": {
          user_id: "user-1",
          created_at: "2024-01-01T00:00:01Z",
          chat_group_id: "group-1",
          role: "user",
          content: "Group 1 message",
          metadata: "{}",
          parts: "[]",
        },
        "msg-2": {
          user_id: "user-1",
          created_at: "2024-01-02T00:00:01Z",
          chat_group_id: "group-2",
          role: "user",
          content: "Group 2 message",
          metadata: "{}",
          parts: "[]",
        },
      },
    };

    const result = tablesToChatJsonList(tables);

    expect(result).toHaveLength(2);

    const group1 = result.find((r) => r.chat_group.id === "group-1");
    const group2 = result.find((r) => r.chat_group.id === "group-2");

    expect(group1?.messages).toHaveLength(1);
    expect(group2?.messages).toHaveLength(1);
  });

  test("returns empty array for empty tables", () => {
    const tables: TablesContent = {};
    const result = tablesToChatJsonList(tables);
    expect(result).toHaveLength(0);
  });
});

describe("roundtrip", () => {
  test("tablesToChatJsonList -> chatJsonToData -> mergeLoadedData preserves data", () => {
    const originalTables: TablesContent = {
      chat_groups: {
        "group-1": {
          user_id: "user-1",
          created_at: "2024-01-01T00:00:00Z",
          title: "Test Chat",
        },
      },
      chat_messages: {
        "msg-1": {
          user_id: "user-1",
          created_at: "2024-01-01T00:00:01Z",
          chat_group_id: "group-1",
          role: "user",
          content: "Hello",
          metadata: "{}",
          parts: "[]",
        },
      },
    };

    const jsonList = tablesToChatJsonList(originalTables);
    const loadedDataList = jsonList.map(chatJsonToData);
    const merged = mergeLoadedData(loadedDataList);

    expect(merged.chat_groups).toEqual(originalTables.chat_groups);
    expect(merged.chat_messages).toEqual(originalTables.chat_messages);
  });

  test("chatJsonToData -> tablesToChatJsonList roundtrip", () => {
    const originalJson: ChatJson = {
      chat_group: {
        id: "group-1",
        user_id: "user-1",
        created_at: "2024-01-01T00:00:00Z",
        title: "Test Chat",
      },
      messages: [
        {
          id: "msg-1",
          user_id: "user-1",
          created_at: "2024-01-01T00:00:01Z",
          chat_group_id: "group-1",
          role: "user",
          content: "Hello",
          metadata: "{}",
          parts: "[]",
        },
      ],
    };

    const loadedData = chatJsonToData(originalJson);
    const tables: TablesContent = {
      chat_groups: loadedData.chat_groups,
      chat_messages: loadedData.chat_messages,
    };
    const jsonList = tablesToChatJsonList(tables);

    expect(jsonList).toHaveLength(1);
    expect(jsonList[0]).toEqual(originalJson);
  });
});

describe("createEmptyLoadedChatData", () => {
  test("returns empty LoadedChatData", () => {
    const empty = createEmptyLoadedChatData();

    expect(empty.chat_groups).toEqual({});
    expect(empty.chat_messages).toEqual({});
  });
});

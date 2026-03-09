import { useEffect } from "react";

import { md2json } from "@hypr/tiptap/shared";
import { buildSegments, ChannelProfile } from "@hypr/transcript";

import { createSessionRow, type Store, STORE_ID, UI } from "@/store/tinybase";

import { SessionPanel } from "./session-panel";

const DEMO_MEMO_MD = `\
**Key takeaways**

- Team agreed on weekly content publishing cadence
- Focus on developer-facing blog posts first
- Review analytics in next sync`;

let _demoMemo: ReturnType<typeof md2json> | null = null;
function getDemoMemo() {
  if (!_demoMemo) {
    _demoMemo = md2json(DEMO_MEMO_MD);
  }
  return _demoMemo;
}

const DEMO_SUMMARY =
  "The team discussed their content strategy and publishing cadence. They agreed to publish one blog post per week, focusing on developer-facing content. The initial topics will cover AI-powered note-taking workflows and integration guides. Analytics review is scheduled for the next sync meeting.";

const DEMO_WORDS = [
  {
    id: "w1",
    text: "Let's ",
    start_ms: 0,
    end_ms: 300,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w2",
    text: "talk ",
    start_ms: 300,
    end_ms: 500,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w3",
    text: "about ",
    start_ms: 500,
    end_ms: 700,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w4",
    text: "the ",
    start_ms: 700,
    end_ms: 850,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w5",
    text: "content ",
    start_ms: 850,
    end_ms: 1100,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w6",
    text: "strategy. ",
    start_ms: 1100,
    end_ms: 1500,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w7",
    text: "I ",
    start_ms: 1600,
    end_ms: 1700,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w8",
    text: "think ",
    start_ms: 1700,
    end_ms: 1900,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w9",
    text: "we ",
    start_ms: 1900,
    end_ms: 2050,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w10",
    text: "should ",
    start_ms: 2050,
    end_ms: 2300,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w11",
    text: "aim ",
    start_ms: 2300,
    end_ms: 2500,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w12",
    text: "for ",
    start_ms: 2500,
    end_ms: 2650,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w13",
    text: "at ",
    start_ms: 2650,
    end_ms: 2800,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w14",
    text: "least ",
    start_ms: 2800,
    end_ms: 3000,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w15",
    text: "one ",
    start_ms: 3000,
    end_ms: 3200,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w16",
    text: "post ",
    start_ms: 3200,
    end_ms: 3400,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w17",
    text: "per ",
    start_ms: 3400,
    end_ms: 3550,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w18",
    text: "week. ",
    start_ms: 3550,
    end_ms: 3900,
    channel: ChannelProfile.DirectMic,
  },

  {
    id: "w19",
    text: "That ",
    start_ms: 4500,
    end_ms: 4700,
    channel: ChannelProfile.RemoteParty,
  },
  {
    id: "w20",
    text: "sounds ",
    start_ms: 4700,
    end_ms: 4950,
    channel: ChannelProfile.RemoteParty,
  },
  {
    id: "w21",
    text: "reasonable. ",
    start_ms: 4950,
    end_ms: 5400,
    channel: ChannelProfile.RemoteParty,
  },
  {
    id: "w22",
    text: "Should ",
    start_ms: 5500,
    end_ms: 5700,
    channel: ChannelProfile.RemoteParty,
  },
  {
    id: "w23",
    text: "we ",
    start_ms: 5700,
    end_ms: 5850,
    channel: ChannelProfile.RemoteParty,
  },
  {
    id: "w24",
    text: "focus ",
    start_ms: 5850,
    end_ms: 6100,
    channel: ChannelProfile.RemoteParty,
  },
  {
    id: "w25",
    text: "on ",
    start_ms: 6100,
    end_ms: 6250,
    channel: ChannelProfile.RemoteParty,
  },
  {
    id: "w26",
    text: "technical ",
    start_ms: 6250,
    end_ms: 6600,
    channel: ChannelProfile.RemoteParty,
  },
  {
    id: "w27",
    text: "content ",
    start_ms: 6600,
    end_ms: 6850,
    channel: ChannelProfile.RemoteParty,
  },
  {
    id: "w28",
    text: "or ",
    start_ms: 6850,
    end_ms: 7000,
    channel: ChannelProfile.RemoteParty,
  },
  {
    id: "w29",
    text: "more ",
    start_ms: 7000,
    end_ms: 7200,
    channel: ChannelProfile.RemoteParty,
  },
  {
    id: "w30",
    text: "general ",
    start_ms: 7200,
    end_ms: 7500,
    channel: ChannelProfile.RemoteParty,
  },
  {
    id: "w31",
    text: "stuff? ",
    start_ms: 7500,
    end_ms: 7900,
    channel: ChannelProfile.RemoteParty,
  },

  {
    id: "w32",
    text: "Developer-facing ",
    start_ms: 8500,
    end_ms: 9000,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w33",
    text: "for ",
    start_ms: 9000,
    end_ms: 9150,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w34",
    text: "now. ",
    start_ms: 9150,
    end_ms: 9450,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w35",
    text: "Things ",
    start_ms: 9550,
    end_ms: 9800,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w36",
    text: "like ",
    start_ms: 9800,
    end_ms: 9950,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w37",
    text: "how ",
    start_ms: 9950,
    end_ms: 10100,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w38",
    text: "to ",
    start_ms: 10100,
    end_ms: 10200,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w39",
    text: "use ",
    start_ms: 10200,
    end_ms: 10400,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w40",
    text: "our ",
    start_ms: 10400,
    end_ms: 10550,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w41",
    text: "API, ",
    start_ms: 10550,
    end_ms: 10800,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w42",
    text: "integration ",
    start_ms: 10800,
    end_ms: 11200,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w43",
    text: "guides, ",
    start_ms: 11200,
    end_ms: 11500,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w44",
    text: "that ",
    start_ms: 11500,
    end_ms: 11650,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w45",
    text: "kind ",
    start_ms: 11650,
    end_ms: 11800,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w46",
    text: "of ",
    start_ms: 11800,
    end_ms: 11900,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w47",
    text: "thing. ",
    start_ms: 11900,
    end_ms: 12300,
    channel: ChannelProfile.DirectMic,
  },

  {
    id: "w48",
    text: "Got ",
    start_ms: 13000,
    end_ms: 13200,
    channel: ChannelProfile.RemoteParty,
  },
  {
    id: "w49",
    text: "it. ",
    start_ms: 13200,
    end_ms: 13500,
    channel: ChannelProfile.RemoteParty,
  },
  {
    id: "w50",
    text: "I ",
    start_ms: 13600,
    end_ms: 13700,
    channel: ChannelProfile.RemoteParty,
  },
  {
    id: "w51",
    text: "can ",
    start_ms: 13700,
    end_ms: 13900,
    channel: ChannelProfile.RemoteParty,
  },
  {
    id: "w52",
    text: "draft ",
    start_ms: 13900,
    end_ms: 14150,
    channel: ChannelProfile.RemoteParty,
  },
  {
    id: "w53",
    text: "the ",
    start_ms: 14150,
    end_ms: 14300,
    channel: ChannelProfile.RemoteParty,
  },
  {
    id: "w54",
    text: "first ",
    start_ms: 14300,
    end_ms: 14500,
    channel: ChannelProfile.RemoteParty,
  },
  {
    id: "w55",
    text: "one ",
    start_ms: 14500,
    end_ms: 14650,
    channel: ChannelProfile.RemoteParty,
  },
  {
    id: "w56",
    text: "on ",
    start_ms: 14650,
    end_ms: 14800,
    channel: ChannelProfile.RemoteParty,
  },
  {
    id: "w57",
    text: "AI-powered ",
    start_ms: 14800,
    end_ms: 15200,
    channel: ChannelProfile.RemoteParty,
  },
  {
    id: "w58",
    text: "note-taking ",
    start_ms: 15200,
    end_ms: 15600,
    channel: ChannelProfile.RemoteParty,
  },
  {
    id: "w59",
    text: "workflows. ",
    start_ms: 15600,
    end_ms: 16100,
    channel: ChannelProfile.RemoteParty,
  },

  {
    id: "w60",
    text: "Perfect. ",
    start_ms: 16800,
    end_ms: 17200,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w61",
    text: "Let's ",
    start_ms: 17300,
    end_ms: 17500,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w62",
    text: "review ",
    start_ms: 17500,
    end_ms: 17750,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w63",
    text: "the ",
    start_ms: 17750,
    end_ms: 17900,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w64",
    text: "analytics ",
    start_ms: 17900,
    end_ms: 18300,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w65",
    text: "in ",
    start_ms: 18300,
    end_ms: 18400,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w66",
    text: "our ",
    start_ms: 18400,
    end_ms: 18550,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w67",
    text: "next ",
    start_ms: 18550,
    end_ms: 18750,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w68",
    text: "sync ",
    start_ms: 18750,
    end_ms: 18950,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w69",
    text: "to ",
    start_ms: 18950,
    end_ms: 19100,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w70",
    text: "see ",
    start_ms: 19100,
    end_ms: 19250,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w71",
    text: "what ",
    start_ms: 19250,
    end_ms: 19400,
    channel: ChannelProfile.DirectMic,
  },
  {
    id: "w72",
    text: "resonates. ",
    start_ms: 19400,
    end_ms: 19900,
    channel: ChannelProfile.DirectMic,
  },
];

const DEMO_SPEAKER_HINTS = [
  {
    wordIndex: 0,
    data: { type: "user_speaker_assignment" as const, human_id: "yujong" },
  },
  {
    wordIndex: 18,
    data: { type: "provider_speaker_index" as const, speaker_index: 1 },
  },
  {
    wordIndex: 31,
    data: { type: "user_speaker_assignment" as const, human_id: "yujong" },
  },
  {
    wordIndex: 47,
    data: { type: "provider_speaker_index" as const, speaker_index: 1 },
  },
  {
    wordIndex: 59,
    data: { type: "user_speaker_assignment" as const, human_id: "yujong" },
  },
];

let _demoSegments: ReturnType<typeof buildSegments> | null = null;
function getDemoSegments() {
  if (!_demoSegments) {
    _demoSegments = buildSegments(DEMO_WORDS, [], DEMO_SPEAKER_HINTS, {
      numSpeakers: 2,
    });
  }
  return _demoSegments;
}

export function PrefilledDemo({ sessionId }: { sessionId: string }) {
  const store = UI.useStore(STORE_ID) as Store | undefined;

  useEffect(() => {
    if (!store) return;
    if (!store.hasRow("sessions", sessionId)) {
      createSessionRow(store, sessionId);
    }
    store.setPartialRow("sessions", sessionId, {
      title: "Content Strategy & Publishing Cadence",
      transcript_segments: JSON.stringify(getDemoSegments()),
      summary: DEMO_SUMMARY,
      memo: JSON.stringify(getDemoMemo()),
      pipeline_status: "done",
      upload_progress: 100,
    });
  }, [store, sessionId]);

  return <SessionPanel sessionId={sessionId} readonly />;
}

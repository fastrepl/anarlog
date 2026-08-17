// Source-informed adaptation of Vexa v0.12.18 gmeet-capture.ts and pcm-capture.ts.
// Licensed under Apache-2.0; see ../THIRD_PARTY_NOTICES.md and ../third-party/VEXA-LICENSE.
(async () => {
  const existing = globalThis.__anlgCapture;
  if (existing) {
    await existing.ready;
    return { streamCount: existing.streamCount() };
  }

  const SAMPLE_RATE = 16000;
  const BLOCK_SIZE = 4096;
  const SILENCE_THRESHOLD = 0.005;
  const RESCAN_MS = 15000;
  const SPEAKER_SCAN_MS = 250;
  const PROCESSOR_NAME = "anlg-pcm-capture";
  const BINDING_NAME = "anlgCapture";
  const SPEAKING_CLASSES = [
    "Oaajhc",
    "HX2H7",
    "wEsLMd",
    "OgVli",
    "speaking",
    "active-speaker",
    "speaker-active",
    "speaking-indicator",
  ];
  const streamIndexes = new Map();
  const connections = new Map();
  const connectingStreamIds = new Set();
  let nextIndex = 0;
  let nextSequence = 1;
  let running = true;
  let activeSpeakers = [];
  const captureStartedAt = performance.now();
  const trackEndsMs = new Map();

  const workletSource = `
class AnlgPcmCapture extends AudioWorkletProcessor {
  constructor() {
    super();
    this.buffer = new Float32Array(${BLOCK_SIZE});
    this.length = 0;
    this.port.onmessage = ({ data }) => {
      if (data !== "flush") return;
      if (this.length > 0) {
        const trailing = this.buffer.slice(0, this.length);
        this.port.postMessage(trailing, [trailing.buffer]);
        this.buffer = new Float32Array(${BLOCK_SIZE});
        this.length = 0;
      }
      this.port.postMessage({ kind: "flushed" });
    };
  }

  process(inputs) {
    const channel = inputs[0] && inputs[0][0];
    if (!channel) return true;
    for (let index = 0; index < channel.length; index += 1) {
      this.buffer[this.length] = channel[index];
      this.length += 1;
      if (this.length === ${BLOCK_SIZE}) {
        this.port.postMessage(this.buffer, [this.buffer.buffer]);
        this.buffer = new Float32Array(${BLOCK_SIZE});
        this.length = 0;
      }
    }
    return true;
  }
}
registerProcessor("${PROCESSOR_NAME}", AnlgPcmCapture);
`;

  const context = new AudioContext({ sampleRate: SAMPLE_RATE });
  const sink = context.createGain();
  sink.gain.value = 0;
  const contextReady = (async () => {
    if (context.sampleRate !== SAMPLE_RATE) {
      throw new Error(
        `expected ${SAMPLE_RATE} Hz, got ${context.sampleRate} Hz`,
      );
    }
    const moduleUrl = URL.createObjectURL(
      new Blob([workletSource], { type: "application/javascript" }),
    );
    try {
      await context.audioWorklet.addModule(moduleUrl);
    } finally {
      URL.revokeObjectURL(moduleUrl);
    }
    await context.resume();
    if (context.state !== "running") {
      throw new Error(`audio context is ${context.state}`);
    }
    sink.connect(context.destination);
  })();

  const toBase64Pcm = (samples) => {
    const bytes = new Uint8Array(samples.length * 2);
    const view = new DataView(bytes.buffer);
    for (let index = 0; index < samples.length; index += 1) {
      const sample = Math.max(-1, Math.min(1, samples[index]));
      const quantized = sample < 0 ? sample * 32768 : sample * 32767;
      view.setInt16(index * 2, Math.round(quantized), true);
    }
    let binary = "";
    for (let index = 0; index < bytes.length; index += 1) {
      binary += String.fromCharCode(bytes[index]);
    }
    return btoa(binary);
  };

  const reportWarning = (scope, error) => {
    const message = error instanceof Error ? error.message : String(error);
    globalThis[BINDING_NAME](
      JSON.stringify({
        v: 1,
        kind: "warning",
        scope,
        message: message.slice(0, 512),
      }),
    );
  };

  const tileName = (tile) => {
    const translated = tile
      .querySelector("span.notranslate")
      ?.textContent?.trim();
    const selfName = tile
      .querySelector("[data-self-name]")
      ?.getAttribute("data-self-name")
      ?.trim();
    const name = translated || selfName;
    if (!name || name.length > 100 || /[\u0000-\u001f\u007f]/u.test(name))
      return undefined;
    return name;
  };

  const isSelfTile = (tile) =>
    tile.hasAttribute("data-self-name") ||
    tile.querySelector("[data-self-name]") !== null;

  const isSpeakingTile = (tile) =>
    SPEAKING_CLASSES.some(
      (className) =>
        tile.classList.contains(className) ||
        tile.querySelector(`.${className}`) !== null,
    );

  const hintForTile = (tile) => {
    if (!tile || isSelfTile(tile)) return {};
    const speakerName = tileName(tile);
    const rawParticipantId = tile.getAttribute("data-participant-id")?.trim();
    const participantId =
      rawParticipantId &&
      rawParticipantId.length <= 256 &&
      !/[\u0000-\u001f\u007f]/u.test(rawParticipantId)
        ? rawParticipantId
        : undefined;
    return {
      ...(speakerName ? { speaker_name: speakerName } : {}),
      ...(participantId ? { participant_id: participantId } : {}),
    };
  };

  const scanSpeakers = () => {
    const speakers = [];
    const seen = new Set();
    for (const tile of document.querySelectorAll("[data-participant-id]")) {
      const participantId = tile.getAttribute("data-participant-id");
      if (
        !participantId ||
        seen.has(participantId) ||
        isSelfTile(tile) ||
        !isSpeakingTile(tile)
      ) {
        continue;
      }
      seen.add(participantId);
      const hint = hintForTile(tile);
      if (hint.speaker_name || hint.participant_id) speakers.push(hint);
    }
    activeSpeakers = speakers;
  };

  const speakerHint = (element) => {
    const tile = element.closest("[data-participant-id]");
    const direct = hintForTile(tile);
    if (direct.speaker_name || direct.participant_id) return direct;
    return activeSpeakers.length === 1 ? activeSpeakers[0] : {};
  };

  const disconnect = async (streamId) => {
    const connection = connections.get(streamId);
    if (!connection) return;
    connections.delete(streamId);
    await connection.flush();
    try {
      connection.source.disconnect();
      connection.worklet.disconnect();
    } catch {}
  };

  const connect = async (element) => {
    const stream = element.srcObject;
    if (
      !(stream instanceof MediaStream) ||
      stream.getAudioTracks().length === 0
    )
      return;
    const participantTile = element.closest("[data-participant-id]");
    if (participantTile && isSelfTile(participantTile)) return;
    if (connections.has(stream.id) || connectingStreamIds.has(stream.id))
      return;
    const track = stream.getAudioTracks()[0];
    let trackEnded = track.readyState === "ended";
    const onTrackEnded = () => {
      trackEnded = true;
      void disconnect(stream.id);
    };
    track.addEventListener("ended", onTrackEnded, { once: true });
    connectingStreamIds.add(stream.id);
    let connected = false;

    try {
      let trackIndex = streamIndexes.get(stream.id);
      if (trackIndex === undefined) {
        trackIndex = nextIndex;
        nextIndex += 1;
        streamIndexes.set(stream.id, trackIndex);
      }

      const connectionStartedAt = Math.floor(
        performance.now() - captureStartedAt,
      );
      let source;
      let worklet;
      try {
        if (!running || trackEnded || track.readyState === "ended") {
          return;
        }

        source = context.createMediaStreamSource(stream);
        worklet = new AudioWorkletNode(context, PROCESSOR_NAME, {
          numberOfInputs: 1,
          numberOfOutputs: 1,
          outputChannelCount: [1],
          channelCount: 1,
          channelCountMode: "explicit",
          channelInterpretation: "speakers",
        });
        source.connect(worklet);
        worklet.connect(sink);

        let samplesSeen = 0;
        let finishFlush;
        worklet.port.onmessage = ({ data }) => {
          if (data?.kind === "flushed") {
            finishFlush?.();
            return;
          }
          if (!(data instanceof Float32Array) || (!running && !finishFlush))
            return;
          const sampledStartMs =
            connectionStartedAt +
            Math.floor((samplesSeen * 1000) / SAMPLE_RATE);
          const startMs = Math.max(
            sampledStartMs,
            trackEndsMs.get(trackIndex) ?? 0,
          );
          samplesSeen += data.length;
          let peak = 0;
          for (let index = 0; index < data.length; index += 1) {
            peak = Math.max(peak, Math.abs(data[index]));
          }
          if (peak <= SILENCE_THRESHOLD) return;
          const durationMs = Math.ceil((data.length * 1000) / SAMPLE_RATE);
          const speaker = speakerHint(element);
          globalThis[BINDING_NAME](
            JSON.stringify({
              v: 1,
              kind: "audio",
              sequence: nextSequence,
              track_index: trackIndex,
              sample_rate: SAMPLE_RATE,
              start_ms: startMs,
              pcm_s16le: toBase64Pcm(data),
              ...speaker,
            }),
          );
          trackEndsMs.set(trackIndex, startMs + durationMs);
          nextSequence += 1;
        };

        const flush = () =>
          new Promise((resolve) => {
            let timeout;
            const finish = () => {
              clearTimeout(timeout);
              finishFlush = undefined;
              resolve();
            };
            finishFlush = finish;
            timeout = setTimeout(finish, 500);
            try {
              worklet.port.postMessage("flush");
            } catch {
              finish();
            }
          });
        connections.set(stream.id, { source, worklet, flush });
        connected = true;
      } catch (error) {
        try {
          source?.disconnect();
          worklet?.disconnect();
        } catch {}
        throw error;
      }
    } finally {
      if (!connected) track.removeEventListener("ended", onTrackEnded);
      connectingStreamIds.delete(stream.id);
    }
  };

  const scan = async () => {
    if (!running) return;
    const pending = [];
    for (const element of document.querySelectorAll("audio, video")) {
      pending.push(
        connect(element).catch((error) => {
          if (running) reportWarning("connect_stream", error);
        }),
      );
    }
    await Promise.all(pending);
  };

  let timer;
  let speakerTimer;
  const ready = (async () => {
    await contextReady;
    if (!running) return;
    scanSpeakers();
    await scan();
    if (running) {
      timer = setInterval(() => void scan(), RESCAN_MS);
      speakerTimer = setInterval(scanSpeakers, SPEAKER_SCAN_MS);
    }
  })();
  globalThis.__anlgCapture = {
    ready,
    streamCount: () => connections.size,
    stop: async () => {
      if (!running) return;
      running = false;
      if (timer !== undefined) clearInterval(timer);
      if (speakerTimer !== undefined) clearInterval(speakerTimer);
      await Promise.all([...connections.keys()].map(disconnect));
      connectingStreamIds.clear();
      try {
        sink.disconnect();
      } catch {}
      await context.close().catch(() => {});
      delete globalThis.__anlgCapture;
    },
  };
  try {
    await ready;
    return { streamCount: connections.size };
  } catch (error) {
    delete globalThis.__anlgCapture;
    try {
      sink.disconnect();
    } catch {}
    await context.close().catch(() => {});
    throw error;
  }
})();

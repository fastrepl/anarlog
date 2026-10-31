package so.anarlog.localspeech

import android.app.Activity
import android.net.Uri
import android.os.ParcelFileDescriptor
import android.os.SystemClock
import android.util.Log
import android.view.WindowManager
import androidx.annotation.RequiresApi
import java.io.File
import java.io.RandomAccessFile
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicBoolean
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.Deferred
import kotlinx.coroutines.async
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.takeWhile
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withTimeout

@RequiresApi(31)
internal class OnDeviceSpeechEngine(
  private val scope: CoroutineScope,
  private val activity: () -> Activity?,
  private val requireForeground: () -> Unit,
  private val emit: (Map<String, Any>) -> Unit,
  private val client: (String) -> SpeechClient,
  private val keepAwake: Boolean = true,
) {
  private val recognitionLock = Mutex()
  private val liveSessions = ConcurrentHashMap<String, LiveSpeech>()

  private val liveStarts = ConcurrentHashMap<String, Deferred<Unit>>()

  fun startLive(id: String, locale: String): Deferred<Unit> {
    val start = scope.async(start = CoroutineStart.LAZY) { startSession(id, locale) }
    synchronized(liveSessions) {
      if (liveStarts.putIfAbsent(id, start) != null) {
        start.cancel()
        throw SpeechFailure("BUSY")
      }
    }
    start.invokeOnCompletion { liveStarts.remove(id, start) }
    start.start()
    return start
  }

  private suspend fun startSession(id: String, locale: String) {
    if (!recognitionLock.tryLock()) throw SpeechFailure("BUSY")
    val recognizer = try { client(locale) } catch (error: Exception) {
      recognitionLock.unlock()
      throw error
    }
    try {
      requireForeground()
      if (withTimeout(30_000) { recognizer.checkStatus() } != "available") {
        throw SpeechFailure("MODEL_REQUIRED")
      }
      val context = currentCoroutineContext()
      synchronized(liveSessions) {
        // Cancellation must cover the handoff from model startup to the live session.
        context.ensureActive()
        requireForeground()
        val session = LiveSpeech(recognizer) { event ->
          emit(event + ("sessionId" to id))
        }
        Log.i("AnarlogLocalSpeech", "On-device live recognition starting")
        val restoreScreen = keepScreenOn()
        liveSessions[id] = session
        session.start(scope) {
          restoreScreen()
          recognitionLock.unlock()
        }
      }
    } catch (error: Exception) {
      recognizer.close()
      recognitionLock.unlock()
      throw error
    }
  }

  suspend fun checkStatus(locale: String): String {
    return client(locale).use { recognizer ->
      withTimeout(30_000) { recognizer.checkStatus() }
    }
  }

  suspend fun downloadModel(locale: String) {
    recognitionLock.withLock {
      requireForeground()
      client(locale).use { recognizer ->
        withTimeout(30 * 60_000L) {
          when (recognizer.checkStatus()) {
            "available" -> return@withTimeout
            "downloadable" -> recognizer.download()
            "downloading" -> throw SpeechFailure("MODEL_DOWNLOADING")
            else -> throw SpeechFailure("MODEL_UNAVAILABLE")
          }
          if (recognizer.checkStatus() != "available") {
            throw SpeechFailure("MODEL_UNAVAILABLE")
          }
        }
      }
    }
  }

  suspend fun transcribeFile(uri: String, locale: String): String {
    return recognitionLock.withLock {
      requireForeground()
      client(locale).use { recognizer ->
        when (withTimeout(30_000) { recognizer.checkStatus() }) {
          "available" -> Unit
          "downloadable" -> throw SpeechFailure("MODEL_REQUIRED")
          "downloading" -> throw SpeechFailure("MODEL_DOWNLOADING")
          else -> throw SpeechFailure("MODEL_UNAVAILABLE")
        }
        val restoreScreen = keepScreenOn()
        try { recognizeFile(recognizer, uri) } finally { restoreScreen() }
      }
    }
  }

  fun sendAudio(id: String, bytes: ByteArray) = liveSessions[id]?.send(bytes) ?: false

  suspend fun finishLive(id: String): Boolean {
    val session = liveSessions[id] ?: return false
    return try { session.finish() } finally { liveSessions.remove(id, session) }
  }

  fun cancelLive(id: String) {
    synchronized(liveSessions) {
      liveStarts[id]?.cancel()
      liveSessions.remove(id)?.cancel()
    }
  }

  fun cancelAll() {
    synchronized(liveSessions) {
      liveStarts.values.forEach { it.cancel() }
      liveSessions.values.forEach { it.cancel() }
      liveSessions.clear()
    }
  }

  private fun keepScreenOn(): () -> Unit {
    if (!keepAwake) return {}
    val activity = activity() ?: throw SpeechFailure("FOREGROUND_REQUIRED")
    val added = AtomicBoolean(false)
    activity.runOnUiThread {
      if (activity.window.attributes.flags and WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON == 0) {
        activity.window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        added.set(true)
      }
    }
    return {
      activity.runOnUiThread {
        if (added.get()) activity.window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
      }
    }
  }

  private suspend fun recognizeFile(recognizer: SpeechClient, uri: String): String {
    val parsed = Uri.parse(uri)
    if (parsed.scheme != "file") throw SpeechFailure("AUDIO_FORMAT")
    val file = File(parsed.path ?: throw SpeechFailure("AUDIO_FORMAT"))
    if (!file.isFile) throw SpeechFailure("AUDIO_MISSING")
    return RandomAccessFile(file, "r").use { input ->
      val wav = try { readPcmWav(input) } catch (error: Exception) {
        throw SpeechFailure("AUDIO_FORMAT")
      }
      input.seek(wav.offset)
      withTimeout(wav.size * 1000 / 32000 + 60_000) {
        coroutineScope {
          val pipe = ParcelFileDescriptor.createPipe()
          val output = ParcelFileDescriptor.AutoCloseOutputStream(pipe[1])
          val fullyWritten = AtomicBoolean(false)
          val writer = launch {
            val buffer = ByteArray(3200)
            var remaining = wav.size
            var written = 0L
            val started = SystemClock.elapsedRealtime()
            output.use {
              while (remaining > 0) {
                val count = minOf(buffer.size.toLong(), remaining).toInt()
                input.readFully(buffer, 0, count)
                output.write(buffer, 0, count)
                remaining -= count
                written += count
                // This SDK's PFD input is streaming and must be fed at real-time speed.
                delay((started + written * 1000 / 32000 - SystemClock.elapsedRealtime()).coerceAtLeast(0))
              }
              fullyWritten.set(true)
            }
          }
          try {
            val text = StringBuilder()
            var completed = false
            recognizer.recognize(pipe[0]).takeWhile { response ->
              when (response) {
                is SpeechResult.Final -> {
                  if (text.length + response.text.length > 2_000_000) throw SpeechFailure("RESPONSE_TOO_LARGE")
                  if (text.isNotEmpty()) text.append(' ')
                  text.append(response.text)
                }
                is SpeechResult.Completed -> completed = true
                else -> Unit
              }
              !completed
            }.collect {}
            if (!completed || !fullyWritten.get()) throw SpeechFailure("INCOMPLETE")
            writer.join()
            text.toString().trim().ifEmpty { throw SpeechFailure("EMPTY") }
          } finally {
            pipe[0].close()
            output.close()
            writer.cancelAndJoin()
          }
        }
      }
    }
  }

}

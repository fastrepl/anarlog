package so.anarlog.localspeech

import android.os.ParcelFileDescriptor
import android.os.SystemClock
import android.util.Log
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicLong
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.Job
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.takeWhile
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull

internal class LiveSpeech(
  private val recognizer: SpeechClient,
  private val emit: (Map<String, Any>) -> Unit,
) {
  private val audio = PcmAudioQueue()
  private val deliveredBytes = AtomicLong(0)
  private val ended = AtomicBoolean(false)
  private val drained = AtomicBoolean(false)
  private val done = CompletableDeferred<Boolean>()
  private var job: Job? = null

  fun start(scope: CoroutineScope, release: () -> Unit) {
    job = scope.launch(start = CoroutineStart.UNDISPATCHED) {
      var complete = false
      var failureCode = "ERR_MLKIT_STREAM_FAILED"
      try {
        complete = recognize()
      } catch (error: Exception) {
        failureCode = if (error is SpeechFailure) "ERR_MLKIT_${error.reason}" else failureCode
        Log.w("AnarlogLocalSpeech", "Live failure: ${error.javaClass.simpleName} code=$failureCode")
        complete = false
      } finally {
        audio.cancel()
        recognizer.close()
        Log.i("AnarlogLocalSpeech", "Live ended: complete=$complete bytes=${deliveredBytes.get()} ended=${ended.get()} drained=${drained.get()}")
        emit(mapOf("type" to if (complete) "completed" else "error", "code" to failureCode))
        done.complete(complete)
        release()
      }
    }
  }

  fun send(bytes: ByteArray): Boolean {
    return !ended.get() && !done.isCompleted && audio.send(bytes)
  }

  suspend fun finish(): Boolean {
    ended.set(true)
    audio.finish()
    return withTimeoutOrNull(90_000) { done.await() } ?: run {
      cancel()
      false
    }
  }

  fun cancel() {
    ended.set(true)
    audio.cancel()
    job?.cancel()
  }

  private suspend fun recognize(): Boolean = coroutineScope {
    val pipe = ParcelFileDescriptor.createPipe()
    val output = ParcelFileDescriptor.AutoCloseOutputStream(pipe[1])
    val writer = launch {
      var nextWriteAt = SystemClock.elapsedRealtime()
      output.use {
        while (true) {
          val buffer = audio.receive() ?: break
          var offset = 0
          while (offset < buffer.size) {
            val count = minOf(3200, buffer.size - offset)
            delay((nextWriteAt - SystemClock.elapsedRealtime()).coerceAtLeast(0))
            output.write(buffer, offset, count)
            offset += count
            deliveredBytes.addAndGet(count.toLong())
            nextWriteAt = SystemClock.elapsedRealtime() + count * 1000L / 32000
          }
        }
        drained.set(true)
      }
    }
    try {
      var completed = false
      var previousFinalMs = 0L
      recognizer.recognize(pipe[0]).takeWhile { response ->
        when (response) {
          is SpeechResult.Partial -> {
            Log.d("AnarlogLocalSpeech", "Live partial received")
            emit(mapOf("type" to "partial", "text" to response.text))
          }
          is SpeechResult.Final -> {
            Log.d("AnarlogLocalSpeech", "Live final received")
            // The published alpha has no word timings. These are segment bounds from audio delivery.
            val endMs = maxOf(previousFinalMs + 1, deliveredBytes.get() * 1000 / 32000)
            emit(mapOf("type" to "final", "text" to response.text, "startMs" to previousFinalMs, "endMs" to endMs))
            previousFinalMs = endMs
          }
          is SpeechResult.Completed -> completed = true
        }
        !completed
      }.collect {}
      completed && ended.get() && drained.get()
    } finally {
      pipe[0].close()
      output.close()
      writer.cancelAndJoin()
    }
  }
}

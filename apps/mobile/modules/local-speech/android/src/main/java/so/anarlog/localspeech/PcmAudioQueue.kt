package so.anarlog.localspeech

import java.util.concurrent.atomic.AtomicLong
import kotlinx.coroutines.channels.Channel

internal class PcmAudioQueue(private val maxBytes: Long = 1_920_000) {
  private val queuedBytes = AtomicLong(0)
  // Bound audio duration, not callback count: model startup can take several seconds.
  private val buffers = Channel<ByteArray>(Channel.UNLIMITED)

  fun send(bytes: ByteArray): Boolean {
    if (bytes.isEmpty() || bytes.size % 2 != 0 || bytes.size > 320_000) return false
    if (queuedBytes.addAndGet(bytes.size.toLong()) > maxBytes) {
      queuedBytes.addAndGet(-bytes.size.toLong())
      return false
    }
    if (buffers.trySend(bytes).isFailure) {
      queuedBytes.addAndGet(-bytes.size.toLong())
      return false
    }
    return true
  }

  suspend fun receive(): ByteArray? = buffers.receiveCatching().getOrNull()?.also {
    queuedBytes.addAndGet(-it.size.toLong())
  }

  fun finish() { buffers.close() }
  fun cancel() { buffers.cancel() }
}

package so.anarlog.localspeech

import kotlinx.coroutines.runBlocking
import org.junit.Assert.*
import org.junit.Test

class PcmAudioQueueTest {
  @Test fun preservesLiveAudioDuringModelWarmupAndDrainsAfterStop() = runBlocking {
    val queue = PcmAudioQueue(maxBytes = 320_000)
    repeat(100) { index -> assertTrue(queue.send(ByteArray(3200) { index.toByte() })) }
    assertFalse(queue.send(ByteArray(2)))
    queue.finish()
    repeat(100) { index -> assertArrayEquals(ByteArray(3200) { index.toByte() }, queue.receive()) }
    assertNull(queue.receive())
    assertFalse(queue.send(ByteArray(2)))
  }
}

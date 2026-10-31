package so.anarlog.localspeech

import java.io.File
import java.io.RandomAccessFile
import java.nio.ByteBuffer
import java.nio.ByteOrder
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test

class PcmWavTest {
  @Test
  fun stripsWavMetadataAndRejectsAudioThatWouldCorruptThePcmStream() {
    val bytes = ByteBuffer.allocate(58).order(ByteOrder.LITTLE_ENDIAN).apply {
      put("RIFF".toByteArray()); putInt(50); put("WAVE".toByteArray())
      put("fmt ".toByteArray()); putInt(16)
      putShort(1); putShort(1); putInt(16000); putInt(32000); putShort(2); putShort(16)
      put("JUNK".toByteArray()); putInt(1); put(42); put(0)
      put("data".toByteArray()); putInt(4); put(byteArrayOf(1, 2, 3, 4))
    }.array()
    val file = File.createTempFile("local-speech", ".wav")
    try {
      file.writeBytes(bytes)
      RandomAccessFile(file, "r").use { input ->
        assertEquals(PcmWav(54, 4), readPcmWav(input))
      }
      for (broken in listOf(
        bytes.copyOf(56),
        bytes.copyOf().also { it[24] = 0 },
        bytes.copyOf().also { it[22] = 2 },
        bytes.copyOf().also { it[50] = 8 },
      )) {
        file.writeBytes(broken)
        RandomAccessFile(file, "r").use { input ->
          assertThrows(IllegalArgumentException::class.java) { readPcmWav(input) }
        }
      }
    } finally {
      file.delete()
    }
  }
}

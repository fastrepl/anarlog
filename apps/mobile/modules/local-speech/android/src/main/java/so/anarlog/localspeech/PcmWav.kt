package so.anarlog.localspeech

import java.io.RandomAccessFile

internal data class PcmWav(val offset: Long, val size: Long)

// The published ML Kit alpha accepts raw PCM, not a WAV header or compressed audio.
internal fun readPcmWav(file: RandomAccessFile): PcmWav {
  fun tag(): String = ByteArray(4).also(file::readFully).toString(Charsets.US_ASCII)
  fun uint(): Long = Integer.reverseBytes(file.readInt()).toLong() and 0xffffffffL
  fun ushort(): Int = java.lang.Short.reverseBytes(file.readShort()).toInt() and 0xffff
  require(file.length() in 44..(512L * 1024 * 1024))
  require(tag() == "RIFF")
  val end = uint() + 8
  require(end <= file.length() && end >= 44)
  require(tag() == "WAVE")
  var validFormat = false
  var audio: PcmWav? = null
  while (file.filePointer + 8 <= end) {
    val type = tag()
    val size = uint()
    val start = file.filePointer
    require(start + size <= end)
    when (type) {
      "fmt " -> {
        require(size >= 16)
        require(ushort() == 1 && ushort() == 1)
        require(uint() == 16000L && uint() == 32000L)
        require(ushort() == 2 && ushort() == 16)
        validFormat = true
      }
      "data" -> {
        require(audio == null && size > 0 && size % 2 == 0L)
        audio = PcmWav(start, size)
      }
    }
    file.seek(start + size + size % 2)
  }
  require(validFormat)
  return requireNotNull(audio)
}

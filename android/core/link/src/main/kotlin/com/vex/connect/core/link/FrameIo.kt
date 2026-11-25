package com.vex.connect.core.link

import javax.net.ssl.SSLSocket
import uniffi.vex_link_ffi.FrameReader

/** Length-prefixed frames over a socket. Reads belong to one thread; writes may come from several. */
internal class FrameIo(private val socket: SSLSocket) {
    private val reader = FrameReader()
    private val input = socket.inputStream
    private val output = socket.outputStream
    private val buffer = ByteArray(4096)
    private val writeLock = Any()

    /** The next complete frame, or null when the other side closed the connection. */
    fun read(): ByteArray? {
        while (true) {
            reader.next()?.let { return it }
            val n = input.read(buffer)
            if (n < 0) return null
            reader.push(buffer.copyOf(n))
        }
    }

    /** [bytes] already carry their length prefix (the native library adds it). */
    fun write(bytes: ByteArray) {
        synchronized(writeLock) {
            output.write(bytes)
            output.flush()
        }
    }

    fun setReadTimeout(ms: Int) {
        socket.soTimeout = ms
    }

    fun close() {
        runCatching { socket.close() }
    }
}

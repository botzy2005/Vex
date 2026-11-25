package com.vex.connect.core.link

import java.net.InetSocketAddress
import java.net.Socket
import java.security.MessageDigest
import java.security.SecureRandom
import java.security.cert.CertificateException
import java.security.cert.X509Certificate
import javax.net.ssl.SSLContext
import javax.net.ssl.SSLSocket
import javax.net.ssl.TrustManager
import javax.net.ssl.X509TrustManager

/**
 * TLS 1.3 to a PC whose certificate was learned from the QR code.
 *
 * There is no certificate authority here: the PC's certificate is self-signed and is accepted only when its SHA-256 equals
 * the fingerprint saved at pairing. Any other certificate, however valid it looks, is refused. (That is why host-name
 * checks are skipped: the pin is stronger than a name.) TLS 1.2 and older are not offered.
 */
internal object PinnedTls {
    const val DEFAULT_PORT = 47821

    fun connect(host: String, port: Int, pinSha256: ByteArray, connectTimeoutMs: Int = 4_000, readTimeoutMs: Int = 15_000): SSLSocket {
        val trust = object : X509TrustManager {
            override fun checkClientTrusted(chain: Array<out X509Certificate>?, authType: String?) {
                throw CertificateException("client certificates are not used")
            }

            override fun checkServerTrusted(chain: Array<out X509Certificate>?, authType: String?) {
                val leaf = chain?.firstOrNull() ?: throw CertificateException("the PC sent no certificate")
                val hash = MessageDigest.getInstance("SHA-256").digest(leaf.encoded)
                if (!MessageDigest.isEqual(hash, pinSha256)) {
                    throw CertificateException("the certificate does not match the paired PC")
                }
            }

            override fun getAcceptedIssuers(): Array<X509Certificate> = emptyArray()
        }
        val context = SSLContext.getInstance("TLSv1.3")
        context.init(null, arrayOf<TrustManager>(trust), SecureRandom())

        val plain = Socket()
        try {
            plain.tcpNoDelay = true
            plain.connect(InetSocketAddress(host, port), connectTimeoutMs)
            plain.soTimeout = connectTimeoutMs
            val tls = context.socketFactory.createSocket(plain, host, port, true) as SSLSocket
            tls.enabledProtocols = arrayOf("TLSv1.3")
            tls.startHandshake()
            tls.soTimeout = readTimeoutMs
            return tls
        } catch (e: Exception) {
            runCatching { plain.close() }
            throw e
        }
    }
}

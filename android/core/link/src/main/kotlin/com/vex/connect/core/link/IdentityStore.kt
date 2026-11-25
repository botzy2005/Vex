package com.vex.connect.core.link

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import java.io.File
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import uniffi.vex_link_ffi.generateSecret

/**
 * This phone's long-term identity key (32 bytes).
 *
 * The key itself is encrypted with an AES-256-GCM key that lives in the Android Keystore (hardware-backed where the phone
 * has it, never exportable). Only the encrypted bytes are written to the app's private storage. If the file is missing or
 * cannot be decrypted (for example the Keystore key was lost), a NEW identity is created and [Loaded.created] is true:
 * PCs that trusted the old identity no longer recognise this phone and it has to be paired again.
 */
class IdentityStore(context: Context) {
    private val file = File(context.applicationContext.filesDir, "vex-identity.bin")

    class Loaded(val secret: ByteArray, val created: Boolean)

    @Synchronized
    fun loadOrCreate(): Loaded {
        read()?.let { return Loaded(it, created = false) }
        val fresh = generateSecret()
        write(fresh)
        return Loaded(fresh, created = true)
    }

    private fun key(): SecretKey {
        val ks = KeyStore.getInstance(ANDROID_KEYSTORE).apply { load(null) }
        (ks.getKey(ALIAS, null) as? SecretKey)?.let { return it }
        val gen = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, ANDROID_KEYSTORE)
        gen.init(
            KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build(),
        )
        return gen.generateKey()
    }

    private fun read(): ByteArray? {
        if (!file.exists()) return null
        return try {
            val raw = file.readBytes()
            val ivLen = raw[0].toInt()
            val iv = raw.copyOfRange(1, 1 + ivLen)
            val ct = raw.copyOfRange(1 + ivLen, raw.size)
            val cipher = Cipher.getInstance(TRANSFORMATION)
            cipher.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, iv))
            cipher.doFinal(ct).takeIf { it.size == 32 }
        } catch (e: Exception) {
            null
        }
    }

    private fun write(secret: ByteArray) {
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.ENCRYPT_MODE, key())
        val iv = cipher.iv
        val ct = cipher.doFinal(secret)
        val tmp = File(file.parentFile, file.name + ".tmp")
        tmp.writeBytes(byteArrayOf(iv.size.toByte()) + iv + ct)
        if (!tmp.renameTo(file)) {
            file.delete()
            check(tmp.renameTo(file)) { "could not save the identity" }
        }
    }

    private companion object {
        const val ANDROID_KEYSTORE = "AndroidKeyStore"
        const val ALIAS = "vex_connect_identity_wrap_v1"
        const val TRANSFORMATION = "AES/GCM/NoPadding"
    }
}

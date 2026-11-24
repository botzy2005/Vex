package com.vex.connect.core.domain.util

import com.vex.connect.core.domain.model.TransferDirection
import com.vex.connect.core.domain.model.TransferRecord
import java.util.Locale

/** Decimal units (kB, MB, GB), the way Windows Explorer and most network tools show sizes. */
fun formatBytes(bytes: Long): String {
    if (bytes < 0) return "–"
    val b = bytes.toDouble()
    return when {
        bytes >= 1_000_000_000L -> String.format(Locale.US, "%.1f GB", b / 1e9)
        bytes >= 1_000_000L -> String.format(Locale.US, "%.1f MB", b / 1e6)
        bytes >= 1_000L -> String.format(Locale.US, "%d kB", Math.round(b / 1e3))
        else -> "$bytes B"
    }
}

fun formatSpeed(bytesPerSecond: Long): String = "${formatBytes(bytesPerSecond)}/s"

const val MAX_DEVICE_NAME = 40

/**
 * Cleans a device name typed by the user: control characters removed, runs of whitespace collapsed, trimmed,
 * limited to [MAX_DEVICE_NAME] characters. Returns null when nothing usable is left.
 */
fun sanitizeDeviceName(raw: String): String? {
    val cleaned = raw
        .replace(Regex("\\s+"), " ")
        .filter { !it.isISOControl() }
        .trim()
        .take(MAX_DEVICE_NAME)
        .trim()
    return cleaned.ifEmpty { null }
}

enum class TransferFilter {
    All, Sent, Received;

    fun apply(list: List<TransferRecord>): List<TransferRecord> = when (this) {
        All -> list
        Sent -> list.filter { it.direction == TransferDirection.Sent }
        Received -> list.filter { it.direction == TransferDirection.Received }
    }
}

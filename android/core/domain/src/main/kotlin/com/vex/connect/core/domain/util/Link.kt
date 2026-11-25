package com.vex.connect.core.domain.util

import com.vex.connect.core.domain.model.ConnectionState
import com.vex.connect.core.domain.model.DeviceLink
import com.vex.connect.core.domain.model.LinkPhase
import com.vex.connect.core.domain.model.NearbyPc
import com.vex.connect.core.domain.model.PairedDevice

/** How long to wait before the next reconnect attempt: 2 s, 4 s, 8 s, 16 s, then 30 s at most. */
fun reconnectDelayMs(failures: Int): Long {
    if (failures <= 0) return 2_000
    val d = 2_000L shl minOf(failures, 4)
    return minOf(d, 30_000L)
}

/** A nearby PC as listed: names that are not unique get the start of their id appended, so two PCs are never confusable. */
data class NearbyLabel(val pc: NearbyPc, val label: String, val paired: Boolean)

fun labelNearby(found: List<NearbyPc>, pairedIds: Set<String>): List<NearbyLabel> {
    val counts = found.groupingBy { it.name.lowercase() }.eachCount()
    return found
        .sortedWith(compareBy({ it.name.lowercase() }, { it.id }))
        .map { pc ->
            val label = if ((counts[pc.name.lowercase()] ?: 0) > 1) "${pc.name} (${pc.id.take(4)})" else pc.name
            NearbyLabel(pc, label, pc.id in pairedIds)
        }
}

/** The headline state shown on Home: the first paired PC that is connected, else connecting, else offline. */
fun overallConnection(devices: List<PairedDevice>, links: Map<String, DeviceLink>): ConnectionState {
    if (devices.isEmpty()) return ConnectionState.NotPaired
    fun phaseOf(d: PairedDevice) = links[d.id]?.phase ?: LinkPhase.Offline
    devices.firstOrNull { phaseOf(it) == LinkPhase.Connected }?.let { return ConnectionState.Connected(it.name) }
    devices.firstOrNull { phaseOf(it) == LinkPhase.Connecting }?.let { return ConnectionState.Connecting(it.name) }
    return ConnectionState.Offline(devices.first().name)
}

/** Bytes to lowercase hex, and back. Used for ids and fingerprints stored as text. */
fun ByteArray.toHex(): String = joinToString("") { "%02x".format(it) }

fun String.hexToBytes(): ByteArray? {
    if (length % 2 != 0 || !all { it in '0'..'9' || it in 'a'..'f' || it in 'A'..'F' }) return null
    return ByteArray(length / 2) { substring(it * 2, it * 2 + 2).toInt(16).toByte() }
}

/** Unix seconds from a millisecond clock. */
fun secondsOf(millis: Long): ULong = (millis / 1000).toULong()

/**
 * Where to try to reach a paired PC, best first: the address announced on the network right now, then the address that
 * worked last time. Duplicates are removed; no address is invented.
 */
fun candidateAddresses(nearby: NearbyPc?, lastAddress: String?, lastPort: Int?, defaultPort: Int): List<Pair<String, Int>> {
    val out = LinkedHashSet<Pair<String, Int>>()
    if (nearby != null) out += nearby.host to nearby.port
    if (lastAddress != null) out += lastAddress to (lastPort ?: defaultPort)
    return out.toList()
}

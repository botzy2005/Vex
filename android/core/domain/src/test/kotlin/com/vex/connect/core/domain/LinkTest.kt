package com.vex.connect.core.domain

import com.vex.connect.core.domain.model.ConnectionState
import com.vex.connect.core.domain.model.DeviceKind
import com.vex.connect.core.domain.model.DeviceLink
import com.vex.connect.core.domain.model.LinkPhase
import com.vex.connect.core.domain.model.NearbyPc
import com.vex.connect.core.domain.model.PairedDevice
import com.vex.connect.core.domain.util.candidateAddresses
import com.vex.connect.core.domain.util.hexToBytes
import com.vex.connect.core.domain.util.labelNearby
import com.vex.connect.core.domain.util.overallConnection
import com.vex.connect.core.domain.util.reconnectDelayMs
import com.vex.connect.core.domain.util.secondsOf
import com.vex.connect.core.domain.util.toHex
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class LinkTest {
    private fun device(id: String, name: String) = PairedDevice(id, name, DeviceKind.Windows, 1, null)

    @Test
    fun reconnectDelayGrowsAndIsCapped() {
        assertEquals(listOf(2_000L, 4_000L, 8_000L, 16_000L, 30_000L, 30_000L), (0..5).map { reconnectDelayMs(it) })
        assertEquals(30_000L, reconnectDelayMs(1_000))
    }

    @Test
    fun duplicateNamesAreToldApartAndPairedOnesAreMarked() {
        val a = NearbyPc("aaaa1111", "Office PC", "10.0.0.2", 1)
        val b = NearbyPc("bbbb2222", "office pc", "10.0.0.3", 1)
        val c = NearbyPc("cccc3333", "Laptop", "10.0.0.4", 1)
        val labels = labelNearby(listOf(a, b, c), setOf("bbbb2222"))
        assertEquals(listOf("Laptop", "Office PC (aaaa)", "office pc (bbbb)"), labels.map { it.label })
        assertEquals(listOf(false, false, true), labels.map { it.paired })
    }

    @Test
    fun homeShowsTheBestStateOfAnyPairedPc() {
        val ds = listOf(device("1", "A"), device("2", "B"))
        assertEquals(ConnectionState.NotPaired, overallConnection(emptyList(), emptyMap()))
        assertEquals(ConnectionState.Offline("A"), overallConnection(ds, emptyMap()))
        assertEquals(
            ConnectionState.Connecting("B"),
            overallConnection(ds, mapOf("2" to DeviceLink(LinkPhase.Connecting))),
        )
        assertEquals(
            ConnectionState.Connected("B"),
            overallConnection(ds, mapOf("1" to DeviceLink(LinkPhase.Connecting), "2" to DeviceLink(LinkPhase.Connected))),
        )
    }

    @Test
    fun hexRoundTripsAndRejectsBadInput() {
        val bytes = byteArrayOf(0, 1, 127, -1, -128)
        assertEquals("00017fff80", bytes.toHex())
        assertArrayEquals(bytes, "00017fff80".hexToBytes())
        assertArrayEquals(bytes, "00017FFF80".hexToBytes())
        assertNull("abc".hexToBytes())
        assertNull("zz".hexToBytes())
    }

    @Test
    fun addressesAreFreshOnesFirstWithoutDuplicates() {
        val nearby = NearbyPc("id", "PC", "192.168.1.9", 5000)
        assertEquals(
            listOf("192.168.1.9" to 5000, "192.168.1.5" to 47821),
            candidateAddresses(nearby, "192.168.1.5", null, 47821),
        )
        assertEquals(listOf("192.168.1.9" to 5000), candidateAddresses(nearby, "192.168.1.9", 5000, 47821))
        assertTrue(candidateAddresses(null, null, null, 47821).isEmpty())
    }

    @Test
    fun secondsFromMillis() {
        assertEquals(1_700_000_000uL, secondsOf(1_700_000_000_999))
    }
}

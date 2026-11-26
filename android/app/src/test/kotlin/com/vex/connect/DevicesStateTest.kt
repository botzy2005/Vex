package com.vex.connect

import com.vex.connect.core.domain.model.DeviceKind
import com.vex.connect.core.domain.model.DeviceLink
import com.vex.connect.core.domain.model.LinkPhase
import com.vex.connect.core.domain.model.NearbyPc
import com.vex.connect.core.domain.model.PairedDevice
import com.vex.connect.diagnostics.formatCrashReport
import com.vex.connect.ui.buildDevicesState
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** Opening Devices must work with nothing paired, nothing found and discovery unavailable (so the list stays empty). */
class DevicesStateTest {
    private val noFingerprint: (PairedDevice) -> String = { error("must not be asked for when there are no devices") }

    @Test fun nothingPairedAndDiscoveryUnavailable() {
        val s = buildDevicesState(emptyList(), emptyMap(), emptyList(), null, null, noFingerprint)
        assertTrue(s.rows.isEmpty())
        assertTrue(s.nearby.isEmpty())
        assertEquals(null, s.phoneName)
        assertEquals(null, s.notice)
    }

    @Test fun nearbyPcsWithTheSameNameAreToldApart() {
        val a = NearbyPc("a".repeat(32), "Desk", "192.168.1.2", 47821)
        val b = NearbyPc("b".repeat(32), "Desk", "192.168.1.3", 47821)
        val s = buildDevicesState(emptyList(), emptyMap(), listOf(a, b), "Nord", null, noFingerprint)
        assertEquals(listOf("Desk (aaaa)", "Desk (bbbb)"), s.nearby.map { it.label })
        assertEquals(2, s.nearby.map { it.pc.id }.toSet().size)
    }

    @Test fun pairedDeviceWithoutALinkIsOffline() {
        val d = PairedDevice("i".repeat(32), "PC", DeviceKind.Windows, 1L, null)
        val s = buildDevicesState(listOf(d), emptyMap(), emptyList(), null, null) { "ab" }
        assertEquals(LinkPhase.Offline, s.rows.single().link.phase)
        assertEquals(DeviceLink(), s.rows.single().link)
    }

    @Test fun crashReportNamesTheExceptionAndTheLine() {
        val e = IllegalStateException("boom")
        val text = formatCrashReport(true, "main", e, "0.2.0", "15", 35, "OnePlus CPH2465", 123L)
        assertTrue(text.contains("java.lang.IllegalStateException: boom"))
        assertTrue(text.contains("DevicesStateTest.kt"))
        assertTrue(text.contains("thread: main"))
        assertTrue(text.contains("uncaught exception"))
    }
}

/** The Devices list puts paired PCs and PCs found on the network in one lazy list: no two rows may share a key. */
class DevicesKeysTest {
    private val id = "9673f611a095b08f3e78bb3e801969b0"
    private fun paired(id: String = this.id) = PairedDevice(id, "Desk PC", DeviceKind.Windows, 1L, null)
    private fun nearby(id: String = this.id) = NearbyPc(id, "Desk PC", "192.168.1.2", 47821)

    @Test fun aPairedPcThatIsAlsoAnnouncingKeepsBothRowsWithDistinctKeys() {
        val s = buildDevicesState(listOf(paired()), emptyMap(), listOf(nearby()), null, null) { "fp" }
        assertEquals(1, s.rows.size)
        assertEquals(1, s.nearby.size)
        assertTrue(s.nearby.single().paired)
        val keys = com.vex.connect.ui.devices.devicesListKeys(s)
        assertEquals(listOf("paired:$id", "nearby:$id"), keys)
        assertEquals(keys.size, keys.toSet().size)
    }

    @Test fun duplicateIdsInEitherSourceCollapseToOneRow() {
        val other = "f".repeat(32)
        val s = buildDevicesState(
            listOf(paired(), paired(), paired(other)),
            emptyMap(),
            listOf(nearby(), nearby(), nearby(other), nearby("0".repeat(32))),
            null, null,
        ) { "fp" }
        assertEquals(listOf(id, other), s.rows.map { it.device.id })
        assertEquals(3, s.nearby.size)
        val keys = com.vex.connect.ui.devices.devicesListKeys(s)
        assertEquals(keys.size, keys.toSet().size)
    }

    @Test fun noticeKeyCannotCollideWithADeviceKey() {
        val n = com.vex.connect.core.domain.model.LinkNotice(7, "removed")
        val s = buildDevicesState(listOf(paired("7")), emptyMap(), listOf(nearby("7")), null, n) { "fp" }
        val keys = com.vex.connect.ui.devices.devicesListKeys(s)
        assertEquals(keys.size, keys.toSet().size)
    }
}

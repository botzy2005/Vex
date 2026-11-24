package com.vex.connect.core.domain

import com.vex.connect.core.domain.model.TransferDirection
import com.vex.connect.core.domain.model.TransferRecord
import com.vex.connect.core.domain.model.TransferStatus
import com.vex.connect.core.domain.util.MAX_DEVICE_NAME
import com.vex.connect.core.domain.util.TransferFilter
import com.vex.connect.core.domain.util.formatBytes
import com.vex.connect.core.domain.util.formatSpeed
import com.vex.connect.core.domain.util.sanitizeDeviceName
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class FormatTest {
    @Test
    fun bytesUseDecimalUnits() {
        assertEquals("0 B", formatBytes(0))
        assertEquals("999 B", formatBytes(999))
        assertEquals("340 kB", formatBytes(340_000))
        assertEquals("1.2 MB", formatBytes(1_200_000))
        assertEquals("2.5 GB", formatBytes(2_500_000_000))
        assertEquals("–", formatBytes(-1))
    }

    @Test
    fun speedAddsPerSecond() {
        assertEquals("1.2 MB/s", formatSpeed(1_200_000))
    }

    @Test
    fun deviceNameIsCleaned() {
        assertEquals("My phone", sanitizeDeviceName("  My \t  phone \n"))
        assertEquals("AB", sanitizeDeviceName("A\u0000B"))
        assertNull(sanitizeDeviceName("   "))
        assertNull(sanitizeDeviceName("\u0007\u0000"))
        assertEquals(MAX_DEVICE_NAME, sanitizeDeviceName("x".repeat(100))!!.length)
    }

    private fun record(id: Long, dir: TransferDirection) = TransferRecord(
        id = id, deviceName = "PC", fileName = "f$id", sizeBytes = 1, direction = dir,
        status = TransferStatus.Completed, startedAtMillis = 0, finishedAtMillis = 1,
    )

    @Test
    fun filtersSelectByDirection() {
        val all = listOf(record(1, TransferDirection.Sent), record(2, TransferDirection.Received), record(3, TransferDirection.Sent))
        assertEquals(listOf(1L, 3L), TransferFilter.Sent.apply(all).map { it.id })
        assertEquals(listOf(2L), TransferFilter.Received.apply(all).map { it.id })
        assertEquals(3, TransferFilter.All.apply(all).size)
    }
}

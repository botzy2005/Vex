package com.vex.connect

import com.vex.connect.ui.devices.lastSeenText
import org.junit.Assert.assertEquals
import org.junit.Test

class LastSeenTest {
    private val now = 10_000_000_000L

    @Test fun neverAndFuture() {
        assertEquals("Never connected", lastSeenText(null, now))
        assertEquals("Seen just now", lastSeenText(now + 5_000, now))
    }

    @Test fun buckets() {
        assertEquals("Seen just now", lastSeenText(now - 30_000, now))
        assertEquals("Seen 5 min ago", lastSeenText(now - 5 * 60_000, now))
        assertEquals("Seen 3 h ago", lastSeenText(now - 3 * 3_600_000L, now))
        assertEquals("Seen 2 d ago", lastSeenText(now - 2 * 86_400_000L, now))
    }
}

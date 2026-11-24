package com.vex.connect.core.data

import androidx.datastore.preferences.core.mutablePreferencesOf
import com.vex.connect.core.domain.model.DeviceKind
import com.vex.connect.core.domain.model.ThemeMode
import com.vex.connect.core.domain.model.TransferDirection
import com.vex.connect.core.domain.model.TransferStatus
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Test

class MappingTest {
    @Test
    fun deviceEntityMapsAndUnknownKindFallsBack() {
        val known = DeviceEntity("a", "Laptop", "Windows", 5, null).toDomain()
        assertEquals(DeviceKind.Windows, known.kind)
        assertEquals(5L, known.pairedAtMillis)
        assertEquals(DeviceKind.Windows, DeviceEntity("b", "X", "Hologram", 1, 2).toDomain().kind)
    }

    @Test
    fun transferEntityMapsAndUnknownValuesAreSafe() {
        val t = TransferEntity(7, "PC", "a.jpg", 10, "Sent", "Completed", 1, 2).toDomain()
        assertEquals(TransferDirection.Sent, t.direction)
        assertEquals(TransferStatus.Completed, t.status)
        val odd = TransferEntity(8, "PC", "b", 1, "Sideways", "Teleported", 1, null).toDomain()
        assertEquals(TransferStatus.Failed, odd.status)
        assertNull(odd.finishedAtMillis)
    }

    @Test
    fun emptyPreferencesGiveDefaults() {
        val s = mutablePreferencesOf().toAppSettings()
        assertEquals(ThemeMode.Dark, s.themeMode)
        assertNull(s.deviceName)
        assertFalse(s.onboardingDone)
    }

    @Test
    fun storedPreferencesAreReadAndDamagedOnesIgnored() {
        val p = mutablePreferencesOf().apply {
            this[SettingKeys.theme] = "Light"
            this[SettingKeys.deviceName] = "  Nord   CE3 "
            this[SettingKeys.onboardingDone] = true
        }
        val s = p.toAppSettings()
        assertEquals(ThemeMode.Light, s.themeMode)
        assertEquals("Nord CE3", s.deviceName)
        assertEquals(true, s.onboardingDone)
        val odd = mutablePreferencesOf().apply { this[SettingKeys.theme] = "Neon" }
        assertEquals(ThemeMode.Dark, odd.toAppSettings().themeMode)
    }
}

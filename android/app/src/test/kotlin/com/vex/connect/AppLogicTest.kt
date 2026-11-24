package com.vex.connect

import com.vex.connect.core.domain.model.ThemeMode
import com.vex.connect.core.domain.model.TransferDirection
import com.vex.connect.core.domain.model.TransferRecord
import com.vex.connect.core.domain.model.TransferStatus
import com.vex.connect.core.domain.util.TransferFilter
import com.vex.connect.core.designsystem.components.Tone
import com.vex.connect.core.domain.model.ConnectionState
import com.vex.connect.navigation.Destination
import com.vex.connect.ui.presentConnection
import com.vex.connect.ui.Stage
import com.vex.connect.ui.buildTransfersState
import com.vex.connect.ui.isDarkTheme
import com.vex.connect.ui.resolveStage
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class AppLogicTest {
    @Test
    fun themeFollowsTheChoice() {
        assertTrue(isDarkTheme(ThemeMode.Dark, systemDark = false))
        assertFalse(isDarkTheme(ThemeMode.Light, systemDark = true))
        assertTrue(isDarkTheme(ThemeMode.System, systemDark = true))
        assertFalse(isDarkTheme(ThemeMode.System, systemDark = false))
    }

    @Test
    fun launchWaitsForBothAnimationAndSettings() {
        assertEquals(Stage.Launch, resolveStage(launchFinished = false, settingsLoaded = true, onboardingDone = true))
        assertEquals(Stage.Launch, resolveStage(launchFinished = true, settingsLoaded = false, onboardingDone = true))
        assertEquals(Stage.Onboarding, resolveStage(true, true, onboardingDone = false))
        assertEquals(Stage.Main, resolveStage(true, true, onboardingDone = true))
    }

    private fun rec(id: Long, dir: TransferDirection) = TransferRecord(
        id, "PC", "f$id", 1, dir, TransferStatus.Completed, 0, 1,
    )

    @Test
    fun transfersStateDistinguishesEmptyHistoryFromEmptyFilter() {
        val none = buildTransfersState(TransferFilter.Sent, emptyList())
        assertTrue(none.historyEmpty && none.items.isEmpty())
        val onlyReceived = buildTransfersState(TransferFilter.Sent, listOf(rec(1, TransferDirection.Received)))
        assertFalse(onlyReceived.historyEmpty)
        assertTrue(onlyReceived.items.isEmpty())
    }

    @Test
    fun bottomBarHasTheFourDestinationsWithUniqueRoutes() {
        val routes = Destination.entries.map { it.route }
        assertEquals(listOf("home", "devices", "transfers", "settings"), routes)
        assertEquals(routes.size, routes.toSet().size)
    }

    @Test
    fun everyConnectionStateHasHonestPresentation() {
        assertEquals("Not connected", presentConnection(ConnectionState.NotPaired).pill)
        assertEquals(Tone.Ok, presentConnection(ConnectionState.Connected("PC")).tone)
        assertEquals(Tone.Neutral, presentConnection(ConnectionState.Offline("PC")).tone)
        // Only an attempt in progress may animate.
        val animated = listOf(
            ConnectionState.NotPaired, ConnectionState.Offline("a"),
            ConnectionState.Connecting("a"), ConnectionState.Connected("a"),
        ).filter { presentConnection(it).pulsing }
        assertEquals(listOf<ConnectionState>(ConnectionState.Connecting("a")), animated)
    }
}

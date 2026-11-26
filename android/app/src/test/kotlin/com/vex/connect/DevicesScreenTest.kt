package com.vex.connect

import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.assertIsDisplayed
import com.vex.connect.core.designsystem.theme.VexTheme
import com.vex.connect.core.domain.model.DeviceKind
import com.vex.connect.core.domain.model.NearbyPc
import com.vex.connect.core.domain.model.PairedDevice
import com.vex.connect.ui.buildDevicesState
import com.vex.connect.ui.devices.DevicesContent
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Opens the real Devices screen (no phone needed). Regression for the crash
 * `IllegalArgumentException: Key "9673f611..." was already used` when a paired PC was also found on the network.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34], qualifiers = "w411dp-h2400dp")
class DevicesScreenTest {
    @get:Rule val rule = createComposeRule()

    private val id = "9673f611a095b08f3e78bb3e801969b0"

    private fun show(paired: List<PairedDevice>, nearby: List<NearbyPc>) {
        val state = buildDevicesState(paired, emptyMap(), nearby, null, null) { "ab12-cd34" }
        rule.setContent {
            VexTheme(darkTheme = true) {
                DevicesContent(state, PaddingValues(), onPair = {}, onRename = { _, _ -> true }, onToggle = {}, onRemove = {}, onDismissNotice = {})
            }
        }
    }

    private fun pc(id: String = this.id, name: String = "Desk PC") = PairedDevice(id, name, DeviceKind.Windows, 1L, null)
    private fun seen(id: String = this.id, name: String = "Desk PC") = NearbyPc(id, name, "192.168.1.2", 47821)

    @Test fun opensWithNothingPairedAndNothingFound() {
        show(emptyList(), emptyList())
        rule.onNodeWithText("No PCs paired").assertIsDisplayed()
        rule.onNodeWithText("Devices").assertIsDisplayed()
    }

    @Test fun opensWhenAPairedPcIsAlsoAnnouncing() {
        show(listOf(pc()), listOf(seen()))
        rule.onNodeWithText("Already paired").assertIsDisplayed()
        // The PC appears twice on purpose: once as paired, once as found on the network.
        rule.onAllNodesWithText("Desk PC").assertCountEquals(2)
    }

    @Test fun opensWithDuplicateEntriesAndKeepsEveryDistinctPc() {
        val other = "f".repeat(32)
        show(listOf(pc(), pc(), pc(other, "Laptop")), listOf(seen(), seen(), seen(other, "Laptop"), seen("0".repeat(32), "Office")))
        rule.onNodeWithText("Office").assertIsDisplayed()
        rule.onAllNodesWithText("Laptop").assertCountEquals(2)
        rule.onAllNodesWithText("Desk PC").assertCountEquals(2)
    }
}

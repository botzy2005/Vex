package com.vex.connect.ui.home

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.ContentPaste
import androidx.compose.material.icons.rounded.Folder
import androidx.compose.material.icons.rounded.Link
import androidx.compose.material.icons.rounded.LinkOff
import androidx.compose.material.icons.rounded.QrCodeScanner
import androidx.compose.material.icons.rounded.BatteryChargingFull
import androidx.compose.material.icons.rounded.NotificationsActive
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.vex.connect.core.designsystem.components.Divider
import com.vex.connect.core.designsystem.components.PlannedTag
import com.vex.connect.core.designsystem.components.SectionLabel
import com.vex.connect.core.designsystem.components.StatusOrb
import com.vex.connect.core.designsystem.components.StatusPill
import com.vex.connect.core.designsystem.components.StepRow
import com.vex.connect.core.designsystem.components.Tone
import com.vex.connect.core.designsystem.components.VexButton
import com.vex.connect.core.designsystem.components.VexCard
import com.vex.connect.core.designsystem.components.VexMark
import com.vex.connect.core.designsystem.theme.Spacing
import com.vex.connect.core.designsystem.theme.VexTheme
import com.vex.connect.ui.AppViewModelFactory
import com.vex.connect.ui.HomeViewModel
import com.vex.connect.ui.presentConnection
import com.vex.connect.core.domain.model.ConnectionState

@Composable
fun HomeScreen(factory: AppViewModelFactory, contentPadding: PaddingValues, onPair: () -> Unit) {
    val vm: HomeViewModel = viewModel(factory = factory)
    val state by vm.state.collectAsStateWithLifecycle()
    val connection = presentConnection(state.connection)
    val notPaired = state.connection is ConnectionState.NotPaired

    LazyColumn(
        contentPadding = PaddingValues(
            start = Spacing.screen,
            end = Spacing.screen,
            top = contentPadding.calculateTopPadding() + Spacing.sm,
            bottom = contentPadding.calculateBottomPadding() + Spacing.xl,
        ),
        verticalArrangement = Arrangement.spacedBy(Spacing.md),
    ) {
        item {
            Row(Modifier.fillMaxWidth().padding(top = Spacing.sm), verticalAlignment = Alignment.CenterVertically) {
                VexMark(size = 30.dp)
                Column(Modifier.weight(1f).padding(start = Spacing.md)) {
                    Text("VEX Connect", style = MaterialTheme.typography.titleLarge)
                    Text("Phone companion for VEX Desktop", style = MaterialTheme.typography.bodySmall, color = VexTheme.extras.textDim)
                }
                StatusPill(connection.pill, connection.tone)
            }
        }
        item {
            VexCard {
                Column(Modifier.fillMaxWidth(), horizontalAlignment = Alignment.CenterHorizontally) {
                    StatusOrb(tone = connection.tone, icon = if (state.connection is ConnectionState.Connected) Icons.Rounded.Link else Icons.Rounded.LinkOff, pulsing = connection.pulsing)
                    Text(
                        connection.title,
                        style = MaterialTheme.typography.titleLarge,
                        modifier = Modifier.padding(top = Spacing.md),
                    )
                    Text(
                        when (state.connection) {
                            is ConnectionState.NotPaired -> "Pair this phone with VEX on your PC. Pairing only works on the same Wi-Fi network."
                            is ConnectionState.Connected -> "Linked over your local network. Nothing is sent or received yet: file transfer arrives in a later version."
                            is ConnectionState.Connecting -> "Looking for your PC on this network."
                            is ConnectionState.Offline -> "Open VEX on your PC, turn VEX Connect on, and make sure both are on the same Wi-Fi."
                        },
                        style = MaterialTheme.typography.bodyMedium,
                        color = VexTheme.extras.textDim,
                        textAlign = TextAlign.Center,
                        modifier = Modifier.padding(top = Spacing.sm),
                    )
                    if (notPaired) {
                        VexButton("Pair a PC", onPair, Modifier.fillMaxWidth().padding(top = Spacing.lg), leading = Icons.Rounded.QrCodeScanner)
                    }
                }
            }
        }
        item {
            Row(horizontalArrangement = Arrangement.spacedBy(Spacing.md)) {
                Stat("Paired PCs", state.pairedCount.toString(), Modifier.weight(1f))
                Stat("Transfers", state.transferCount.toString(), Modifier.weight(1f))
            }
        }
        if (notPaired) item { SectionLabel("How pairing works") }
        if (notPaired) item {
            VexCard {
                Column(verticalArrangement = Arrangement.spacedBy(Spacing.lg)) {
                    StepRow(1, "Open VEX on your PC", "Choose Add device. VEX shows a QR code that expires after a couple of minutes.")
                    StepRow(2, "Scan the code with this phone", "The code carries only temporary pairing information, never a permanent secret.")
                    StepRow(3, "Confirm on both screens", "You compare a 6-digit code on the PC and the phone before either trusts the other.")
                }
            }
        }
        item { SectionLabel("Planned features") }
        item {
            VexCard {
                Column {
                    Feature(Icons.Rounded.Folder, "Files", "Send and receive over Wi-Fi")
                    Divider()
                    Feature(Icons.Rounded.ContentPaste, "Clipboard", "Share copied text, only when you choose")
                    Divider()
                    Feature(Icons.Rounded.NotificationsActive, "Notifications", "See phone alerts on your PC, per app")
                    Divider()
                    Feature(Icons.Rounded.BatteryChargingFull, "Phone status", "Battery and connection at a glance")
                }
            }
        }
    }
}

@Composable
private fun Stat(label: String, value: String, modifier: Modifier = Modifier) {
    VexCard(modifier) {
        Text(value, style = MaterialTheme.typography.headlineMedium)
        Text(label, style = MaterialTheme.typography.bodySmall, color = VexTheme.extras.textDim)
    }
}

@Composable
private fun Feature(icon: ImageVector, title: String, subtitle: String) {
    Row(
        Modifier.fillMaxWidth().padding(vertical = Spacing.md),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(Spacing.md),
    ) {
        Icon(icon, contentDescription = null, tint = MaterialTheme.colorScheme.primary, modifier = Modifier.padding(end = 0.dp))
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.titleSmall)
            Text(subtitle, style = MaterialTheme.typography.bodySmall, color = VexTheme.extras.textDim)
        }
        PlannedTag()
    }
}

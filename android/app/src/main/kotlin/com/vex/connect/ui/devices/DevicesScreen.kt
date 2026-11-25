package com.vex.connect.ui.devices

import android.os.Build
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Computer
import androidx.compose.material.icons.rounded.PhoneAndroid
import androidx.compose.material.icons.rounded.QrCodeScanner
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.vex.connect.core.designsystem.components.EmptyState
import com.vex.connect.core.designsystem.components.ScreenHeader
import com.vex.connect.core.designsystem.components.SectionLabel
import com.vex.connect.core.designsystem.components.StatusPill
import com.vex.connect.core.designsystem.components.Tone
import com.vex.connect.core.designsystem.components.VexButton
import com.vex.connect.core.designsystem.components.VexCard
import com.vex.connect.core.designsystem.components.VexTextButton
import com.vex.connect.core.designsystem.theme.Spacing
import com.vex.connect.core.designsystem.theme.VexTheme
import com.vex.connect.core.domain.model.LinkPhase
import com.vex.connect.ui.AppViewModelFactory
import com.vex.connect.ui.DeviceRowState
import com.vex.connect.ui.DevicesViewModel

private sealed interface Dialog {
    data class Rename(val row: DeviceRowState) : Dialog
    data class Remove(val row: DeviceRowState) : Dialog
}

/** "Just now", "5 min ago", "3 h ago", or "2 d ago". */
fun lastSeenText(lastSeenMillis: Long?, nowMillis: Long): String {
    if (lastSeenMillis == null) return "Never connected"
    val s = ((nowMillis - lastSeenMillis) / 1000).coerceAtLeast(0)
    return when {
        s < 60 -> "Seen just now"
        s < 3600 -> "Seen ${s / 60} min ago"
        s < 86_400 -> "Seen ${s / 3600} h ago"
        else -> "Seen ${s / 86_400} d ago"
    }
}

@Composable
fun DevicesScreen(factory: AppViewModelFactory, contentPadding: PaddingValues, onPair: () -> Unit) {
    val vm: DevicesViewModel = viewModel(factory = factory)
    val state by vm.state.collectAsStateWithLifecycle()
    var dialog by remember { mutableStateOf<Dialog?>(null) }

    LazyColumn(
        contentPadding = PaddingValues(
            start = Spacing.screen,
            end = Spacing.screen,
            top = contentPadding.calculateTopPadding(),
            bottom = contentPadding.calculateBottomPadding() + Spacing.xl,
        ),
        verticalArrangement = Arrangement.spacedBy(Spacing.md),
    ) {
        item { ScreenHeader("Devices", subtitle = "The PCs this phone trusts") }
        state.notice?.let { n ->
            item(key = "notice-${n.id}") {
                VexCard {
                    Text(n.text, style = MaterialTheme.typography.bodyMedium)
                    VexTextButton("Dismiss", { vm.dismissNotice(n.id) })
                }
            }
        }
        item { SectionLabel("This phone") }
        item {
            VexCard {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(Spacing.md)) {
                    Icon(Icons.Rounded.PhoneAndroid, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
                    Column(Modifier.weight(1f)) {
                        Text(state.phoneName ?: Build.MODEL, style = MaterialTheme.typography.titleMedium)
                        Text(
                            "Android ${Build.VERSION.RELEASE} · ${Build.MANUFACTURER.replaceFirstChar { it.uppercase() }} ${Build.MODEL}",
                            style = MaterialTheme.typography.bodySmall,
                            color = VexTheme.extras.textDim,
                        )
                    }
                    StatusPill(if (state.rows.isEmpty()) "Not paired" else "Paired", if (state.rows.isEmpty()) Tone.Warn else Tone.Ok)
                }
            }
        }
        item { SectionLabel("Paired PCs") }
        if (state.rows.isEmpty()) {
            item {
                VexCard {
                    EmptyState(
                        icon = Icons.Rounded.Computer,
                        title = "No PCs paired",
                        body = "Turn on VEX Connect in VEX on your PC (Settings, Phone), then scan the QR code it shows.",
                    )
                }
            }
        } else {
            items(state.rows, key = { it.device.id }) { row ->
                DeviceCard(
                    row = row,
                    onRename = { dialog = Dialog.Rename(row) },
                    onToggle = { if (row.link.held) vm.reconnect(row.device.id) else vm.disconnect(row.device.id) },
                    onRemove = { dialog = Dialog.Remove(row) },
                )
            }
        }
        item { VexButton("Pair a PC", onPair, Modifier.fillMaxWidth(), leading = Icons.Rounded.QrCodeScanner) }
        item { SectionLabel("Found on this network") }
        if (state.nearby.isEmpty()) {
            item {
                Text(
                    "No VEX PCs announced on this Wi-Fi. They appear here while VEX Connect is on.",
                    style = MaterialTheme.typography.bodySmall,
                    color = VexTheme.extras.textDim,
                )
            }
        } else {
            items(state.nearby, key = { it.pc.id }) { n ->
                VexCard {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(Spacing.md)) {
                        Icon(Icons.Rounded.Computer, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
                        Column(Modifier.weight(1f)) {
                            Text(n.label, style = MaterialTheme.typography.titleSmall)
                            Text(
                                if (n.paired) "Already paired" else "Not paired. Scan its QR code to pair.",
                                style = MaterialTheme.typography.bodySmall,
                                color = VexTheme.extras.textDim,
                            )
                        }
                    }
                }
            }
        }
    }

    when (val d = dialog) {
        is Dialog.Rename -> RenameDialog(d.row.device.name, onDismiss = { dialog = null }) { name ->
            if (vm.rename(d.row.device.id, name)) dialog = null
        }
        is Dialog.Remove -> AlertDialog(
            onDismissRequest = { dialog = null },
            title = { Text("Remove ${d.row.device.name}?") },
            text = { Text("This phone will forget the PC, and the PC is told to forget this phone if it is connected. You will need to pair again to reconnect.") },
            confirmButton = { VexTextButton("Remove", { vm.remove(d.row.device.id); dialog = null }) },
            dismissButton = { VexTextButton("Cancel", { dialog = null }) },
        )
        null -> {}
    }
}

@Composable
private fun DeviceCard(row: DeviceRowState, onRename: () -> Unit, onToggle: () -> Unit, onRemove: () -> Unit) {
    val (pill, tone) = when (row.link.phase) {
        LinkPhase.Connected -> "Connected" to Tone.Ok
        LinkPhase.Connecting -> "Connecting" to Tone.Accent
        LinkPhase.Offline -> "Offline" to Tone.Neutral
    }
    VexCard {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(Spacing.md)) {
            Icon(Icons.Rounded.Computer, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
            Column(Modifier.weight(1f)) {
                Text(row.device.name, style = MaterialTheme.typography.titleMedium)
                Text(
                    lastSeenText(row.device.lastSeenMillis, System.currentTimeMillis()),
                    style = MaterialTheme.typography.bodySmall,
                    color = VexTheme.extras.textDim,
                )
            }
            StatusPill(pill, tone)
        }
        if (row.link.phase != LinkPhase.Connected && row.link.detail != null) {
            Text(
                row.link.detail,
                style = MaterialTheme.typography.bodySmall,
                color = VexTheme.extras.textDim,
                modifier = Modifier.padding(top = Spacing.sm),
            )
        }
        if (row.fingerprint.isNotEmpty()) {
            Text(
                "Fingerprint ${row.fingerprint}",
                style = MaterialTheme.typography.bodySmall,
                color = VexTheme.extras.textFaint,
                modifier = Modifier.padding(top = Spacing.xs),
            )
        }
        Row(Modifier.fillMaxWidth().padding(top = Spacing.sm), horizontalArrangement = Arrangement.End) {
            VexTextButton("Rename", onRename)
            VexTextButton(if (row.link.held) "Reconnect" else "Disconnect", onToggle)
            VexTextButton("Remove", onRemove)
        }
    }
}

@Composable
private fun RenameDialog(current: String, onDismiss: () -> Unit, onSave: (String) -> Unit) {
    var text by remember { mutableStateOf(current) }
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text("Rename PC") },
        text = {
            OutlinedTextField(value = text, onValueChange = { text = it.take(40) }, singleLine = true, label = { Text("Name") })
        },
        confirmButton = { VexTextButton("Save", { onSave(text) }, enabled = text.isNotBlank()) },
        dismissButton = { VexTextButton("Cancel", onDismiss) },
    )
}

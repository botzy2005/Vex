package com.vex.connect.ui.transfers

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.ArrowDownward
import androidx.compose.material.icons.rounded.ArrowUpward
import androidx.compose.material.icons.rounded.SwapVert
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
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
import com.vex.connect.core.designsystem.components.StatusPill
import com.vex.connect.core.designsystem.components.Tone
import com.vex.connect.core.designsystem.components.VexCard
import com.vex.connect.core.designsystem.components.VexTextButton
import com.vex.connect.core.designsystem.theme.Spacing
import com.vex.connect.core.designsystem.theme.VexTheme
import com.vex.connect.core.domain.model.TransferDirection
import com.vex.connect.core.domain.model.TransferRecord
import com.vex.connect.core.domain.model.TransferStatus
import com.vex.connect.core.domain.util.TransferFilter
import com.vex.connect.core.domain.util.formatBytes
import com.vex.connect.ui.AppViewModelFactory
import com.vex.connect.ui.TransfersViewModel

@Composable
fun TransfersScreen(factory: AppViewModelFactory, contentPadding: PaddingValues) {
    val vm: TransfersViewModel = viewModel(factory = factory)
    val state by vm.state.collectAsStateWithLifecycle()
    var confirmClear by remember { mutableStateOf(false) }

    LazyColumn(
        contentPadding = PaddingValues(
            start = Spacing.screen,
            end = Spacing.screen,
            top = contentPadding.calculateTopPadding(),
            bottom = contentPadding.calculateBottomPadding() + Spacing.xl,
        ),
        verticalArrangement = Arrangement.spacedBy(Spacing.md),
    ) {
        item { ScreenHeader("Transfers", subtitle = "Files sent to and received from your PC") }
        item {
            Row(Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(Spacing.sm)) {
                TransferFilter.entries.forEach { f ->
                    FilterChip(selected = state.filter == f, onClick = { vm.setFilter(f) }, label = { Text(f.name) })
                }
                Row(Modifier.weight(1f), horizontalArrangement = Arrangement.End) {
                    VexTextButton("Clear", onClick = { confirmClear = true }, enabled = !state.historyEmpty)
                }
            }
        }
        if (state.items.isEmpty()) {
            item {
                VexCard {
                    if (state.historyEmpty) {
                        EmptyState(
                            icon = Icons.Rounded.SwapVert,
                            title = "No transfers yet",
                            body = "File transfer is not part of this version. When it arrives, every file you send or receive will be listed here with its progress and result.",
                        )
                    } else {
                        EmptyState(
                            icon = Icons.Rounded.SwapVert,
                            title = "Nothing matches this filter",
                            body = "Choose All to see the whole history.",
                        )
                    }
                }
            }
        } else {
            items(state.items, key = { it.id }) { TransferRow(it) }
        }
    }

    if (confirmClear) {
        AlertDialog(
            onDismissRequest = { confirmClear = false },
            title = { Text("Clear transfer history?") },
            text = { Text("This removes the list on this phone. It does not delete any received files.") },
            confirmButton = {
                VexTextButton("Clear", onClick = { vm.clearHistory(); confirmClear = false })
            },
            dismissButton = { VexTextButton("Cancel", onClick = { confirmClear = false }) },
        )
    }
}

@Composable
private fun TransferRow(t: TransferRecord) {
    VexCard {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(Spacing.md)) {
            Icon(
                if (t.direction == TransferDirection.Sent) Icons.Rounded.ArrowUpward else Icons.Rounded.ArrowDownward,
                contentDescription = t.direction.name,
                tint = MaterialTheme.colorScheme.primary,
            )
            Column(Modifier.weight(1f)) {
                Text(t.fileName, style = MaterialTheme.typography.titleSmall, maxLines = 1)
                Text("${t.deviceName} · ${formatBytes(t.sizeBytes)}", style = MaterialTheme.typography.bodySmall, color = VexTheme.extras.textDim)
            }
            val (label, tone) = when (t.status) {
                TransferStatus.Completed -> "Done" to Tone.Ok
                TransferStatus.Failed -> "Failed" to Tone.Danger
                TransferStatus.Cancelled -> "Cancelled" to Tone.Neutral
                TransferStatus.Paused -> "Paused" to Tone.Warn
                TransferStatus.InProgress -> "Sending" to Tone.Accent
                TransferStatus.Queued -> "Queued" to Tone.Neutral
            }
            StatusPill(label, tone)
        }
    }
}

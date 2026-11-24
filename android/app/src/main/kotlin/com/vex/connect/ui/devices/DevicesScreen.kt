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
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.vex.connect.core.designsystem.components.EmptyState
import com.vex.connect.core.designsystem.components.ScreenHeader
import com.vex.connect.core.designsystem.components.SectionLabel
import com.vex.connect.core.designsystem.components.StatusPill
import com.vex.connect.core.designsystem.components.Tone
import com.vex.connect.core.designsystem.components.VexCard
import com.vex.connect.core.designsystem.theme.Spacing
import com.vex.connect.core.designsystem.theme.VexTheme
import com.vex.connect.ui.AppViewModelFactory
import com.vex.connect.ui.DevicesViewModel

@Composable
fun DevicesScreen(factory: AppViewModelFactory, contentPadding: PaddingValues) {
    val vm: DevicesViewModel = viewModel(factory = factory)
    val devices by vm.devices.collectAsStateWithLifecycle()
    val savedName by vm.phoneName.collectAsStateWithLifecycle()

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
        item { SectionLabel("This phone") }
        item {
            VexCard {
                Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(Spacing.md)) {
                    Icon(Icons.Rounded.PhoneAndroid, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
                    Column(Modifier.weight(1f)) {
                        Text(savedName ?: Build.MODEL, style = MaterialTheme.typography.titleMedium)
                        Text(
                            "Android ${Build.VERSION.RELEASE} · ${Build.MANUFACTURER.replaceFirstChar { it.uppercase() }} ${Build.MODEL}",
                            style = MaterialTheme.typography.bodySmall,
                            color = VexTheme.extras.textDim,
                        )
                    }
                    StatusPill("Not paired", Tone.Warn)
                }
            }
        }
        item { SectionLabel("Paired PCs") }
        if (devices.isEmpty()) {
            item {
                VexCard {
                    EmptyState(
                        icon = Icons.Rounded.Computer,
                        title = "No PCs paired",
                        body = "When pairing arrives, the PCs you trust will be listed here, with their connection state and when they were last seen. You will be able to rename or remove each one.",
                    )
                }
            }
        } else {
            items(devices, key = { it.id }) { device ->
                VexCard {
                    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(Spacing.md)) {
                        Icon(Icons.Rounded.Computer, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
                        Column(Modifier.weight(1f)) {
                            Text(device.name, style = MaterialTheme.typography.titleMedium)
                            Text(device.kind.name, style = MaterialTheme.typography.bodySmall, color = VexTheme.extras.textDim)
                        }
                        StatusPill("Offline", Tone.Neutral)
                    }
                }
            }
        }
    }
}

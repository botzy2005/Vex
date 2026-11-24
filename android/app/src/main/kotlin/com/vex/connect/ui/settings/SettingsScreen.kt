package com.vex.connect.ui.settings

import android.os.Build
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.BatteryChargingFull
import androidx.compose.material.icons.rounded.ContentPaste
import androidx.compose.material.icons.rounded.Edit
import androidx.compose.material.icons.rounded.Info
import androidx.compose.material.icons.rounded.NotificationsActive
import androidx.compose.material.icons.rounded.PhoneAndroid
import androidx.compose.material.icons.rounded.Slideshow
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.FilterChip
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import com.vex.connect.core.designsystem.components.Divider
import com.vex.connect.core.designsystem.components.PlannedTag
import com.vex.connect.core.designsystem.components.ScreenHeader
import com.vex.connect.core.designsystem.components.SectionLabel
import com.vex.connect.core.designsystem.components.SettingRow
import com.vex.connect.core.designsystem.components.VexCard
import com.vex.connect.core.designsystem.components.VexTextButton
import com.vex.connect.core.designsystem.theme.Spacing
import com.vex.connect.core.designsystem.theme.VexTheme
import com.vex.connect.core.domain.model.ThemeMode
import com.vex.connect.core.domain.util.MAX_DEVICE_NAME
import com.vex.connect.ui.AppViewModelFactory
import com.vex.connect.ui.SettingsViewModel

@Composable
fun SettingsScreen(factory: AppViewModelFactory, contentPadding: PaddingValues) {
    val vm: SettingsViewModel = viewModel(factory = factory)
    val settings by vm.settings.collectAsStateWithLifecycle()
    val nameError by vm.nameError.collectAsStateWithLifecycle()
    var editingName by remember { mutableStateOf(false) }
    val context = LocalContext.current
    val version = remember {
        runCatching { context.packageManager.getPackageInfo(context.packageName, 0).versionName }.getOrNull() ?: "unknown"
    }

    LazyColumn(
        contentPadding = PaddingValues(
            start = Spacing.screen,
            end = Spacing.screen,
            top = contentPadding.calculateTopPadding(),
            bottom = contentPadding.calculateBottomPadding() + Spacing.xl,
        ),
        verticalArrangement = Arrangement.spacedBy(Spacing.md),
    ) {
        item { ScreenHeader("Settings") }
        item { SectionLabel("Appearance") }
        item {
            VexCard {
                Text("Theme", style = androidx.compose.material3.MaterialTheme.typography.titleSmall)
                Row(Modifier.padding(top = Spacing.sm), horizontalArrangement = Arrangement.spacedBy(Spacing.sm)) {
                    ThemeMode.entries.forEach { mode ->
                        FilterChip(
                            selected = settings.themeMode == mode,
                            onClick = { vm.setTheme(mode) },
                            label = { Text(mode.name) },
                        )
                    }
                }
            }
        }
        item { SectionLabel("This phone") }
        item {
            VexCard {
                SettingRow(
                    title = "Device name",
                    subtitle = (settings.deviceName ?: Build.MODEL) + if (settings.deviceName == null) " (phone model)" else "",
                    icon = Icons.Rounded.PhoneAndroid,
                    onClick = { vm.clearNameError(); editingName = true },
                    trailing = {
                        androidx.compose.material3.Icon(
                            Icons.Rounded.Edit,
                            contentDescription = "Edit device name",
                            tint = VexTheme.extras.textDim,
                        )
                    },
                )
                Divider()
                SettingRow(
                    title = "Show introduction again",
                    subtitle = "Replay the welcome screens",
                    icon = Icons.Rounded.Slideshow,
                    onClick = { vm.showIntroAgain() },
                )
            }
        }
        item { SectionLabel("Planned settings") }
        item {
            VexCard {
                Column {
                    SettingRow(title = "Clipboard sync", subtitle = "Per-device, off by default", icon = Icons.Rounded.ContentPaste, trailing = { PlannedTag() })
                    Divider()
                    SettingRow(title = "Notification sync", subtitle = "Per-app filters; needs Android's notification access", icon = Icons.Rounded.NotificationsActive, trailing = { PlannedTag() })
                    Divider()
                    SettingRow(title = "Stay connected", subtitle = "Optional background service, off by default", icon = Icons.Rounded.BatteryChargingFull, trailing = { PlannedTag() })
                }
            }
        }
        item { SectionLabel("About") }
        item {
            VexCard {
                SettingRow(title = "VEX Connect", subtitle = "Version $version · Stage 1: app foundation", icon = Icons.Rounded.Info)
                Divider()
                SettingRow(
                    title = "Network access",
                    subtitle = "This build does not request the Internet permission. It cannot connect to anything.",
                )
            }
        }
    }

    if (editingName) {
        var text by remember { mutableStateOf(settings.deviceName ?: Build.MODEL) }
        AlertDialog(
            onDismissRequest = { editingName = false },
            title = { Text("Device name") },
            text = {
                Column {
                    Text(
                        "This is the name your PC will show for this phone once pairing is available.",
                        style = androidx.compose.material3.MaterialTheme.typography.bodySmall,
                        color = VexTheme.extras.textDim,
                    )
                    OutlinedTextField(
                        value = text,
                        onValueChange = { if (it.length <= MAX_DEVICE_NAME) text = it },
                        singleLine = true,
                        isError = nameError != null,
                        supportingText = { nameError?.let { Text(it) } },
                        modifier = Modifier.fillMaxWidth().padding(top = Spacing.md),
                    )
                }
            },
            confirmButton = { VexTextButton("Save", onClick = { if (vm.saveName(text)) editingName = false }) },
            dismissButton = {
                Row {
                    VexTextButton("Use phone model", onClick = { vm.resetName(); editingName = false })
                    VexTextButton("Cancel", onClick = { editingName = false })
                }
            },
        )
    }
}

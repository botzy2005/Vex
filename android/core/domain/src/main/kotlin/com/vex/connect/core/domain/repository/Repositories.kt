package com.vex.connect.core.domain.repository

import com.vex.connect.core.domain.model.AppSettings
import com.vex.connect.core.domain.model.PairedDevice
import com.vex.connect.core.domain.model.ThemeMode
import com.vex.connect.core.domain.model.TransferRecord
import kotlinx.coroutines.flow.Flow

interface SettingsRepository {
    val settings: Flow<AppSettings>
    suspend fun setThemeMode(mode: ThemeMode)
    /** Stores a cleaned-up name, or clears it (use the phone's model name) when null. */
    suspend fun setDeviceName(name: String?)
    suspend fun setOnboardingDone(done: Boolean)
}

interface DeviceRepository {
    val devices: Flow<List<PairedDevice>>
}

interface TransferRepository {
    /** Newest first. */
    val transfers: Flow<List<TransferRecord>>
    suspend fun clearHistory()
}

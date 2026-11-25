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
    suspend fun snapshot(): List<PairedDevice>
    suspend fun get(id: String): PairedDevice?
    /** Adds the device, or replaces the one with the same id (pairing again after a removal). */
    suspend fun upsert(device: PairedDevice)
    suspend fun rename(id: String, name: String)
    suspend fun remove(id: String)
    suspend fun clearAll()
    /** Records a successful contact: when, and where the PC answered. */
    suspend fun touch(id: String, seenMillis: Long, address: String, port: Int)
}

interface TransferRepository {
    /** Newest first. */
    val transfers: Flow<List<TransferRecord>>
    suspend fun clearHistory()
}

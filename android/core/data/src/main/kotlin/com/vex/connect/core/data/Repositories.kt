package com.vex.connect.core.data

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.emptyPreferences
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import com.vex.connect.core.domain.model.AppSettings
import com.vex.connect.core.domain.model.PairedDevice
import com.vex.connect.core.domain.model.ThemeMode
import com.vex.connect.core.domain.model.TransferRecord
import com.vex.connect.core.domain.repository.DeviceRepository
import com.vex.connect.core.domain.repository.SettingsRepository
import com.vex.connect.core.domain.repository.TransferRepository
import com.vex.connect.core.domain.util.sanitizeDeviceName
import java.io.IOException
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.catch
import kotlinx.coroutines.flow.map

private val Context.settingsStore: DataStore<Preferences> by preferencesDataStore(name = "vex_settings")

internal object SettingKeys {
    val theme = stringPreferencesKey("theme_mode")
    val deviceName = stringPreferencesKey("device_name")
    val onboardingDone = booleanPreferencesKey("onboarding_done")
}

/** Pure mapping, so it can be unit-tested without Android. A damaged or unknown value falls back to the default. */
internal fun Preferences.toAppSettings(): AppSettings = AppSettings(
    themeMode = this[SettingKeys.theme]?.let { name -> ThemeMode.entries.firstOrNull { it.name == name } } ?: ThemeMode.Dark,
    deviceName = this[SettingKeys.deviceName]?.let(::sanitizeDeviceName),
    onboardingDone = this[SettingKeys.onboardingDone] ?: false,
)

class DataStoreSettingsRepository(context: Context) : SettingsRepository {
    private val store = context.applicationContext.settingsStore

    override val settings: Flow<AppSettings> = store.data
        .catch { e -> if (e is IOException) emit(emptyPreferences()) else throw e }
        .map { it.toAppSettings() }

    override suspend fun setThemeMode(mode: ThemeMode) {
        store.edit { it[SettingKeys.theme] = mode.name }
    }

    override suspend fun setDeviceName(name: String?) {
        val cleaned = name?.let(::sanitizeDeviceName)
        store.edit { if (cleaned == null) it.remove(SettingKeys.deviceName) else it[SettingKeys.deviceName] = cleaned }
    }

    override suspend fun setOnboardingDone(done: Boolean) {
        store.edit { it[SettingKeys.onboardingDone] = done }
    }
}

class RoomDeviceRepository(private val dao: DeviceDao) : DeviceRepository {
    override val devices: Flow<List<PairedDevice>> = dao.observeAll().map { list -> list.map { it.toDomain() } }
    override suspend fun snapshot(): List<PairedDevice> = dao.all().map { it.toDomain() }
    override suspend fun get(id: String): PairedDevice? = dao.find(id)?.toDomain()
    override suspend fun upsert(device: PairedDevice) = dao.upsert(device.toEntity())
    override suspend fun rename(id: String, name: String) = dao.rename(id, name)
    override suspend fun remove(id: String) = dao.remove(id)
    override suspend fun clearAll() = dao.clear()
    override suspend fun touch(id: String, seenMillis: Long, address: String, port: Int) = dao.touch(id, seenMillis, address, port)
}

class RoomTransferRepository(private val dao: TransferDao) : TransferRepository {
    override val transfers: Flow<List<TransferRecord>> = dao.observeAll().map { list -> list.map { it.toDomain() } }
    override suspend fun clearHistory() = dao.clear()
}

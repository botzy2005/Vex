package com.vex.connect.ui

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.vex.connect.AppContainer
import com.vex.connect.core.domain.model.AppSettings
import com.vex.connect.core.domain.model.PairedDevice
import com.vex.connect.core.domain.model.ThemeMode
import com.vex.connect.core.domain.model.TransferRecord
import com.vex.connect.core.domain.repository.DeviceRepository
import com.vex.connect.core.domain.repository.SettingsRepository
import com.vex.connect.core.domain.repository.TransferRepository
import com.vex.connect.core.domain.util.TransferFilter
import com.vex.connect.core.domain.util.sanitizeDeviceName
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.combine
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

private val SHARE = SharingStarted.WhileSubscribed(5_000)

/** Settings as the whole app needs them; null until the saved values have been read. */
class RootViewModel(private val settingsRepo: SettingsRepository) : ViewModel() {
    val settings: StateFlow<AppSettings?> = settingsRepo.settings.stateIn(viewModelScope, SHARE, null)
    suspend fun finishOnboarding() = settingsRepo.setOnboardingDone(true)
}

data class HomeUiState(val pairedCount: Int = 0, val transferCount: Int = 0)

class HomeViewModel(devices: DeviceRepository, transfers: TransferRepository) : ViewModel() {
    val state: StateFlow<HomeUiState> = combine(devices.devices, transfers.transfers) { d, t ->
        HomeUiState(pairedCount = d.size, transferCount = t.size)
    }.stateIn(viewModelScope, SHARE, HomeUiState())
}

class DevicesViewModel(devices: DeviceRepository, settings: SettingsRepository) : ViewModel() {
    val devices: StateFlow<List<PairedDevice>> = devices.devices.stateIn(viewModelScope, SHARE, emptyList())
    val phoneName: StateFlow<String?> =
        settings.settings.map { it.deviceName }.stateIn(viewModelScope, SHARE, null)
}

data class TransfersUiState(
    val filter: TransferFilter = TransferFilter.All,
    val items: List<TransferRecord> = emptyList(),
    /** True when there is no history at all (as opposed to the filter hiding everything). */
    val historyEmpty: Boolean = true,
)

/** Pure, so it is unit-tested without Android. */
fun buildTransfersState(filter: TransferFilter, all: List<TransferRecord>) =
    TransfersUiState(filter = filter, items = filter.apply(all), historyEmpty = all.isEmpty())

class TransfersViewModel(private val repo: TransferRepository) : ViewModel() {
    private val filter = MutableStateFlow(TransferFilter.All)
    val state: StateFlow<TransfersUiState> =
        combine(filter, repo.transfers) { f, all -> buildTransfersState(f, all) }
            .stateIn(viewModelScope, SHARE, TransfersUiState())

    fun setFilter(f: TransferFilter) {
        filter.value = f
    }

    fun clearHistory() {
        viewModelScope.launch { repo.clearHistory() }
    }
}

class SettingsViewModel(private val repo: SettingsRepository) : ViewModel() {
    val settings: StateFlow<AppSettings> = repo.settings.stateIn(viewModelScope, SHARE, AppSettings())
    private val _nameError = MutableStateFlow<String?>(null)
    val nameError: StateFlow<String?> = _nameError.asStateFlow()

    fun setTheme(mode: ThemeMode) {
        viewModelScope.launch { repo.setThemeMode(mode) }
    }

    /** Returns true when the name was accepted (the dialog can close). */
    fun saveName(raw: String): Boolean {
        val cleaned = sanitizeDeviceName(raw)
        if (cleaned == null) {
            _nameError.value = "Enter a name"
            return false
        }
        _nameError.value = null
        viewModelScope.launch { repo.setDeviceName(cleaned) }
        return true
    }

    fun resetName() {
        _nameError.value = null
        viewModelScope.launch { repo.setDeviceName(null) }
    }

    fun clearNameError() {
        _nameError.value = null
    }

    fun showIntroAgain() {
        viewModelScope.launch { repo.setOnboardingDone(false) }
    }
}

class AppViewModelFactory(private val c: AppContainer) : ViewModelProvider.Factory {
    @Suppress("UNCHECKED_CAST")
    override fun <T : ViewModel> create(modelClass: Class<T>): T {
        val vm: ViewModel = when {
            modelClass.isAssignableFrom(RootViewModel::class.java) -> RootViewModel(c.settings)
            modelClass.isAssignableFrom(HomeViewModel::class.java) -> HomeViewModel(c.devices, c.transfers)
            modelClass.isAssignableFrom(DevicesViewModel::class.java) -> DevicesViewModel(c.devices, c.settings)
            modelClass.isAssignableFrom(TransfersViewModel::class.java) -> TransfersViewModel(c.transfers)
            modelClass.isAssignableFrom(SettingsViewModel::class.java) -> SettingsViewModel(c.settings)
            else -> throw IllegalArgumentException("Unknown ViewModel: ${modelClass.name}")
        }
        return vm as T
    }
}

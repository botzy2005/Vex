package com.vex.connect

import android.app.Application
import android.content.Context
import android.os.Build
import com.vex.connect.core.data.DataStoreSettingsRepository
import com.vex.connect.core.data.RoomDeviceRepository
import com.vex.connect.core.data.RoomTransferRepository
import com.vex.connect.core.data.VexDatabase
import com.vex.connect.core.domain.repository.DeviceRepository
import com.vex.connect.core.domain.repository.SettingsRepository
import com.vex.connect.core.domain.repository.TransferRepository
import com.vex.connect.core.link.IdentityStore
import com.vex.connect.core.link.LinkManager
import com.vex.connect.core.link.NsdDiscovery
import com.vex.connect.core.link.PairingRunner
import com.vex.connect.diagnostics.CrashRecorder
import kotlinx.coroutines.CoroutineExceptionHandler
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch

/** Hand-written dependency container, created once. No DI framework is needed at this size. */
class AppContainer(context: Context, val crashes: CrashRecorder) {
    private val appContext = context.applicationContext
    private val database by lazy { VexDatabase.create(appContext) }
    // An error in background work is recorded for Settings, Diagnostics, instead of ending the app. The other
    // coroutines keep running (SupervisorJob); anything that needs the failed piece retries on its own.
    private val appScope = CoroutineScope(
        SupervisorJob() + Dispatchers.Default + CoroutineExceptionHandler { _, e -> crashes.record(e, fatal = false) },
    )

    val settings: SettingsRepository by lazy { DataStoreSettingsRepository(appContext) }
    val devices: DeviceRepository by lazy { RoomDeviceRepository(database.deviceDao()) }
    val transfers: TransferRepository by lazy { RoomTransferRepository(database.transferDao()) }

    /** This phone's identity. If the Keystore lost the old one, PCs that trusted it can no longer recognise this phone. */
    private val identity by lazy {
        IdentityStore(appContext).loadOrCreate().also { loaded ->
            if (loaded.created) appScope.launch { devices.clearAll() }
        }
    }
    private val identitySecret: () -> ByteArray = { identity.secret }
    private val phoneName: suspend () -> String = { settings.settings.first().deviceName ?: Build.MODEL }

    val discovery: NsdDiscovery by lazy { NsdDiscovery(appContext, onError = { crashes.record(it, fatal = false) }) }
    val links: LinkManager by lazy { LinkManager(appScope, devices, identitySecret, discovery, phoneName) }
    val pairing: PairingRunner by lazy { PairingRunner(appScope, identitySecret, phoneName, devices) }
}

class VexConnectApp : Application() {
    lateinit var container: AppContainer
        private set

    override fun onCreate() {
        super.onCreate()
        val crashes = CrashRecorder(this).also { it.install() }
        container = AppContainer(this, crashes)
    }
}

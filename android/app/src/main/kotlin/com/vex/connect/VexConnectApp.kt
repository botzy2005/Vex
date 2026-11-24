package com.vex.connect

import android.app.Application
import android.content.Context
import com.vex.connect.core.data.DataStoreSettingsRepository
import com.vex.connect.core.data.RoomDeviceRepository
import com.vex.connect.core.data.RoomTransferRepository
import com.vex.connect.core.data.VexDatabase
import com.vex.connect.core.domain.repository.DeviceRepository
import com.vex.connect.core.domain.repository.SettingsRepository
import com.vex.connect.core.domain.repository.TransferRepository

/** Hand-written dependency container: three repositories, created once. No DI framework is needed at this size. */
class AppContainer(context: Context) {
    private val appContext = context.applicationContext
    private val database by lazy { VexDatabase.create(appContext) }

    val settings: SettingsRepository by lazy { DataStoreSettingsRepository(appContext) }
    val devices: DeviceRepository by lazy { RoomDeviceRepository(database.deviceDao()) }
    val transfers: TransferRepository by lazy { RoomTransferRepository(database.transferDao()) }
}

class VexConnectApp : Application() {
    lateinit var container: AppContainer
        private set

    override fun onCreate() {
        super.onCreate()
        container = AppContainer(this)
    }
}

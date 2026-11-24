package com.vex.connect.core.data

import android.content.Context
import androidx.room.Database
import androidx.room.Room
import androidx.room.RoomDatabase

/**
 * Local structured data: paired devices and transfer history. Stage 1 only reads these tables (they are empty);
 * the networking stages add the writes. Private keys never go in this database (Stage 2 uses the Android Keystore).
 */
@Database(entities = [DeviceEntity::class, TransferEntity::class], version = 1, exportSchema = true)
abstract class VexDatabase : RoomDatabase() {
    abstract fun deviceDao(): DeviceDao
    abstract fun transferDao(): TransferDao

    companion object {
        fun create(context: Context): VexDatabase =
            Room.databaseBuilder(context.applicationContext, VexDatabase::class.java, "vex-connect.db").build()
    }
}

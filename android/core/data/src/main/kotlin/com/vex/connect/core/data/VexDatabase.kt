package com.vex.connect.core.data

import android.content.Context
import androidx.room.Database
import androidx.room.Room
import androidx.room.RoomDatabase
import androidx.room.migration.Migration
import androidx.sqlite.db.SupportSQLiteDatabase

/**
 * Local structured data: paired devices and transfer history. Private keys never go in this database: the phone's
 * identity key lives in an Android-Keystore-encrypted file (see core/link IdentityStore). What is stored per PC is public
 * information: its public key, the fingerprint of its TLS certificate and where it was last reached.
 */
@Database(entities = [DeviceEntity::class, TransferEntity::class], version = 2, exportSchema = true)
abstract class VexDatabase : RoomDatabase() {
    abstract fun deviceDao(): DeviceDao
    abstract fun transferDao(): TransferDao

    companion object {
        fun create(context: Context): VexDatabase =
            Room.databaseBuilder(context.applicationContext, VexDatabase::class.java, "vex-connect.db")
                .addMigrations(MIGRATION_1_2)
                .build()
    }
}

/**
 * Version 1 (Stage 1) never stored a paired device, so the table is rebuilt with the new columns instead of altered.
 * Transfer history is untouched.
 */
val MIGRATION_1_2 = object : Migration(1, 2) {
    override fun migrate(db: SupportSQLiteDatabase) {
        db.execSQL("DROP TABLE IF EXISTS `paired_devices`")
        db.execSQL(
            "CREATE TABLE IF NOT EXISTS `paired_devices` (`id` TEXT NOT NULL, `name` TEXT NOT NULL, `kind` TEXT NOT NULL, " +
                "`paired_at` INTEGER NOT NULL, `last_seen` INTEGER, `public_key` TEXT NOT NULL, `tls_fingerprint` TEXT NOT NULL, " +
                "`last_address` TEXT, `last_port` INTEGER, PRIMARY KEY(`id`))",
        )
    }
}

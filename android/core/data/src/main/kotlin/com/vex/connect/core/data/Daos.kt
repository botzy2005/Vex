package com.vex.connect.core.data

import androidx.room.Dao
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import kotlinx.coroutines.flow.Flow

@Dao
interface DeviceDao {
    @Query("SELECT * FROM paired_devices ORDER BY paired_at DESC")
    fun observeAll(): Flow<List<DeviceEntity>>

    @Query("SELECT * FROM paired_devices ORDER BY paired_at DESC")
    suspend fun all(): List<DeviceEntity>

    @Query("SELECT * FROM paired_devices WHERE id = :id")
    suspend fun find(id: String): DeviceEntity?

    @Insert(onConflict = OnConflictStrategy.REPLACE)
    suspend fun upsert(device: DeviceEntity)

    @Query("UPDATE paired_devices SET name = :name WHERE id = :id")
    suspend fun rename(id: String, name: String)

    @Query("DELETE FROM paired_devices WHERE id = :id")
    suspend fun remove(id: String)

    @Query("DELETE FROM paired_devices")
    suspend fun clear()

    @Query("UPDATE paired_devices SET last_seen = :seen, last_address = :address, last_port = :port WHERE id = :id")
    suspend fun touch(id: String, seen: Long, address: String, port: Int)
}

@Dao
interface TransferDao {
    @Query("SELECT * FROM transfers ORDER BY started_at DESC")
    fun observeAll(): Flow<List<TransferEntity>>

    @Query("DELETE FROM transfers")
    suspend fun clear()
}

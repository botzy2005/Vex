package com.vex.connect.core.data

import androidx.room.Dao
import androidx.room.Query
import kotlinx.coroutines.flow.Flow

@Dao
interface DeviceDao {
    @Query("SELECT * FROM paired_devices ORDER BY paired_at DESC")
    fun observeAll(): Flow<List<DeviceEntity>>
}

@Dao
interface TransferDao {
    @Query("SELECT * FROM transfers ORDER BY started_at DESC")
    fun observeAll(): Flow<List<TransferEntity>>

    @Query("DELETE FROM transfers")
    suspend fun clear()
}

package com.vex.connect.core.data

import androidx.room.ColumnInfo
import androidx.room.Entity
import androidx.room.PrimaryKey
import com.vex.connect.core.domain.model.DeviceKind
import com.vex.connect.core.domain.model.PairedDevice
import com.vex.connect.core.domain.model.TransferDirection
import com.vex.connect.core.domain.model.TransferRecord
import com.vex.connect.core.domain.model.TransferStatus

@Entity(tableName = "paired_devices")
data class DeviceEntity(
    @PrimaryKey val id: String,
    val name: String,
    /** [DeviceKind] name. Stored as text so adding a value later needs no migration. */
    val kind: String,
    @ColumnInfo(name = "paired_at") val pairedAt: Long,
    @ColumnInfo(name = "last_seen") val lastSeen: Long?,
)

@Entity(tableName = "transfers")
data class TransferEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    @ColumnInfo(name = "device_name") val deviceName: String,
    @ColumnInfo(name = "file_name") val fileName: String,
    @ColumnInfo(name = "size_bytes") val sizeBytes: Long,
    val direction: String,
    val status: String,
    @ColumnInfo(name = "started_at") val startedAt: Long,
    @ColumnInfo(name = "finished_at") val finishedAt: Long?,
)

/** Unknown stored text (for example from a newer app version) falls back to a safe value instead of crashing. */
private inline fun <reified T : Enum<T>> String.toEnumOr(default: T): T =
    enumValues<T>().firstOrNull { it.name == this } ?: default

fun DeviceEntity.toDomain() = PairedDevice(
    id = id,
    name = name,
    kind = kind.toEnumOr(DeviceKind.Windows),
    pairedAtMillis = pairedAt,
    lastSeenMillis = lastSeen,
)

fun TransferEntity.toDomain() = TransferRecord(
    id = id,
    deviceName = deviceName,
    fileName = fileName,
    sizeBytes = sizeBytes,
    direction = direction.toEnumOr(TransferDirection.Received),
    status = status.toEnumOr(TransferStatus.Failed),
    startedAtMillis = startedAt,
    finishedAtMillis = finishedAt,
)

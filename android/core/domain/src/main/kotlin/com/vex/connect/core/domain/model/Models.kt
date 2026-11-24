package com.vex.connect.core.domain.model

enum class ThemeMode { System, Dark, Light }

/** What the user has chosen on this phone. */
data class AppSettings(
    val themeMode: ThemeMode = ThemeMode.Dark,
    /** The name this phone will show to a PC. Null means "use the phone's model name". */
    val deviceName: String? = null,
    val onboardingDone: Boolean = false,
)

enum class DeviceKind { Windows, Android }

/** A PC (or later another phone) this phone has been paired with. Nothing creates these before Stage 2. */
data class PairedDevice(
    val id: String,
    val name: String,
    val kind: DeviceKind,
    val pairedAtMillis: Long,
    val lastSeenMillis: Long?,
)

enum class TransferDirection { Sent, Received }

enum class TransferStatus { Queued, InProgress, Paused, Completed, Failed, Cancelled }

/** One line of transfer history. Nothing creates these before Stage 3. */
data class TransferRecord(
    val id: Long,
    val deviceName: String,
    val fileName: String,
    val sizeBytes: Long,
    val direction: TransferDirection,
    val status: TransferStatus,
    val startedAtMillis: Long,
    val finishedAtMillis: Long?,
)

/**
 * How this phone relates to a PC. Only [NotPaired] can occur in Stage 1 (there is no networking yet); the other
 * states exist so the UI components are built and tested for them before the networking stages need them.
 */
sealed interface ConnectionState {
    data object NotPaired : ConnectionState
    data class Offline(val deviceName: String) : ConnectionState
    data class Connecting(val deviceName: String) : ConnectionState
    data class Connected(val deviceName: String) : ConnectionState
}

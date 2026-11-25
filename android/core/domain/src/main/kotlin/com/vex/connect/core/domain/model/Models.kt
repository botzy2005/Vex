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

/** A PC (or later another phone) this phone has been paired with. */
data class PairedDevice(
    /** 32 hex characters: the first half of SHA-256 over the PC's public key. */
    val id: String,
    val name: String,
    val kind: DeviceKind,
    val pairedAtMillis: Long,
    val lastSeenMillis: Long?,
    /** Lowercase hex of the PC's 32-byte public key (public information; no secret is ever stored here). */
    val publicKeyHex: String = "",
    /** Lowercase hex of SHA-256 over the PC's TLS certificate; the phone accepts only that certificate. */
    val tlsFingerprintHex: String = "",
    /** Where the PC was last reached, tried first next time. */
    val lastAddress: String? = null,
    val lastPort: Int? = null,
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
 * How this phone relates to a PC. [NotPaired] until a PC is paired; then the state of the link to the PC.
 */
sealed interface ConnectionState {
    data object NotPaired : ConnectionState
    data class Offline(val deviceName: String) : ConnectionState
    data class Connecting(val deviceName: String) : ConnectionState
    data class Connected(val deviceName: String) : ConnectionState
}

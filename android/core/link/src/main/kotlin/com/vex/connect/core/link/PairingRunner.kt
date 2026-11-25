package com.vex.connect.core.link

import com.vex.connect.core.domain.model.DeviceKind
import com.vex.connect.core.domain.model.PairedDevice
import com.vex.connect.core.domain.model.PairingUi
import com.vex.connect.core.domain.repository.DeviceRepository
import com.vex.connect.core.domain.util.secondsOf
import com.vex.connect.core.domain.util.toHex
import java.io.IOException
import javax.net.ssl.SSLException
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import uniffi.vex_link_ffi.LinkFailure
import uniffi.vex_link_ffi.PairingClient
import uniffi.vex_link_ffi.PairingStatus
import uniffi.vex_link_ffi.Ticket
import uniffi.vex_link_ffi.deviceId
import uniffi.vex_link_ffi.parseTicket

/**
 * One pairing attempt, from the scanned (or pasted) code to a saved device.
 *
 * Nothing is saved until BOTH the PC and this phone's user have accepted the same 6-digit code. Cancelling, a lost
 * connection, a refusal on either side or an expired code all end with nothing saved.
 */
class PairingRunner(
    private val scope: CoroutineScope,
    private val identitySecret: () -> ByteArray,
    private val phoneName: suspend () -> String,
    private val devices: DeviceRepository,
    private val clockMillis: () -> Long = System::currentTimeMillis,
) {
    private val _state = MutableStateFlow<PairingUi>(PairingUi.Idle)
    val state: StateFlow<PairingUi> = _state.asStateFlow()

    private class Live(
        val client: PairingClient,
        val io: FrameIo,
        val ticket: Ticket,
        val address: Pair<String, Int>,
    ) {
        @Volatile var answeredYes: Boolean? = null
        @Volatile var saved = false
        val evalLock = Mutex()
    }

    @Volatile private var live: Live? = null
    private var job: Job? = null

    /** Begin pairing with the text of a scanned QR code. Any attempt already running is abandoned. */
    fun start(scanned: String) {
        cancel()
        _state.value = PairingUi.Connecting
        job = scope.launch(Dispatchers.IO) { run(scanned.trim()) }
    }

    /** The user's answer to "does the code match?". */
    fun answer(accept: Boolean) {
        val l = live ?: return
        scope.launch(Dispatchers.IO) {
            try {
                l.answeredYes = accept
                l.io.write(l.client.confirm(accept))
                evaluate(l)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Exception) {
                fail("Pairing stopped: the connection to the PC was lost.")
            }
        }
    }

    /** Abandon the attempt and go back to idle. */
    fun cancel() {
        job?.cancel()
        job = null
        live?.io?.close()
        live = null
        _state.value = PairingUi.Idle
    }

    private fun isTerminal(s: PairingUi) = s is PairingUi.Done || s is PairingUi.Declined || s is PairingUi.Failed

    private fun fail(message: String) {
        if (!isTerminal(_state.value) && _state.value !is PairingUi.Idle) _state.value = PairingUi.Failed(message)
    }

    private fun friendly(e: LinkFailure): String {
        val m = e.message.orEmpty()
        return when {
            "expired" in m -> "This code has expired. Ask the PC for a new one."
            "not a VEX pairing code" in m -> "That isn't a VEX pairing code."
            else -> "That code can't be used: $m"
        }
    }

    private suspend fun run(text: String) {
        var io: FrameIo? = null
        try {
            val now = secondsOf(clockMillis())
            val ticket = try {
                parseTicket(text, now)
            } catch (e: LinkFailure) {
                fail(friendly(e))
                return
            }
            val client = try {
                PairingClient(identitySecret(), text, phoneName(), now)
            } catch (e: LinkFailure) {
                fail(friendly(e))
                return
            }
            val (socket, address, securityFailure) = connectAny(ticket)
            if (socket == null) {
                fail(
                    if (securityFailure) "Security check failed: the device that answered is not the PC that showed this code."
                    else "Couldn't reach ${ticket.pcName}. Check that this phone and the PC are on the same Wi-Fi network.",
                )
                return
            }
            val framed = FrameIo(socket)
            io = framed
            // The user needs time to compare the codes; the PC enforces its own limit.
            framed.setReadTimeout(100_000)
            val l = Live(client, framed, ticket, address!!)
            live = l
            framed.write(client.firstMessage())
            while (true) {
                val frame = framed.read()
                if (frame == null) {
                    fail("The PC closed the connection. Start again with a new code.")
                    return
                }
                for (reply in client.handle(frame)) framed.write(reply)
                if (evaluate(l)) return
            }
        } catch (e: CancellationException) {
            throw e
        } catch (e: LinkFailure) {
            fail("Pairing failed. Start again with a new code.")
        } catch (e: IOException) {
            fail("The connection to the PC was lost. Start again with a new code.")
        } finally {
            io?.close()
            live = null
        }
    }

    private data class Connected(val socket: javax.net.ssl.SSLSocket?, val address: Pair<String, Int>?, val securityFailure: Boolean)

    private fun connectAny(ticket: Ticket): Connected {
        var security = false
        for (host in ticket.addresses) {
            try {
                val s = PinnedTls.connect(host, ticket.port.toInt(), ticket.tlsFingerprint, connectTimeoutMs = 3_000)
                return Connected(s, host to ticket.port.toInt(), false)
            } catch (e: SSLException) {
                security = true
            } catch (e: IOException) {
                // try the next address
            }
        }
        return Connected(null, null, security)
    }

    /** Looks at the native session and updates the screen. Returns true when the attempt is over. */
    private suspend fun evaluate(l: Live): Boolean = l.evalLock.withLock {
        when (val s = l.client.status()) {
            is PairingStatus.WaitingForUser -> {
                if (l.answeredYes == null) _state.value = PairingUi.Confirm(s.code, s.peerName)
                false
            }
            is PairingStatus.WaitingForPeer -> {
                val cur = _state.value
                val (code, name) = if (cur is PairingUi.Confirm) cur.code to cur.pcName else "" to ""
                if (cur is PairingUi.Confirm) _state.value = PairingUi.WaitingForPc(code, name)
                false
            }
            is PairingStatus.Paired -> {
                if (!l.saved) {
                    l.saved = true
                    val now = clockMillis()
                    devices.upsert(
                        PairedDevice(
                            id = deviceId(s.peerPublic),
                            name = s.peerName,
                            kind = DeviceKind.Windows,
                            pairedAtMillis = now,
                            lastSeenMillis = now,
                            publicKeyHex = s.peerPublic.toHex(),
                            tlsFingerprintHex = l.ticket.tlsFingerprint.toHex(),
                            lastAddress = l.address.first,
                            lastPort = l.address.second,
                        ),
                    )
                    _state.value = PairingUi.Done(s.peerName)
                }
                l.io.close()
                true
            }
            is PairingStatus.Declined -> {
                _state.value = PairingUi.Declined(byPhone = l.answeredYes == false)
                l.io.close()
                true
            }
            is PairingStatus.Handshaking -> false
        }
    }
}

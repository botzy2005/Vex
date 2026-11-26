package com.vex.connect.diagnostics

import android.content.Context
import android.os.Build
import android.util.Log
import java.io.File
import java.io.PrintWriter
import java.io.StringWriter

/** The text of one crash report. Pure, so it is unit-tested. */
fun formatCrashReport(
    fatal: Boolean,
    thread: String,
    error: Throwable,
    versionName: String,
    androidRelease: String,
    sdk: Int,
    model: String,
    timeMillis: Long,
): String {
    val trace = StringWriter().also { error.printStackTrace(PrintWriter(it)) }.toString()
    return buildString {
        appendLine(if (fatal) "VEX Connect closed unexpectedly (uncaught exception)" else "VEX Connect recovered from an error in the background")
        appendLine("time: $timeMillis (unix ms)")
        appendLine("app: $versionName")
        appendLine("android: $androidRelease (API $sdk), $model")
        appendLine("thread: $thread")
        appendLine()
        append(trace)
    }
}

/**
 * Keeps the stack trace of the last crash in the app's private storage so it can be read (and copied) from
 * Settings, Diagnostics, without a computer. Nothing is sent anywhere. The report contains the exception, the call
 * stack, the app and Android versions and the phone model; no names, addresses or keys.
 */
class CrashRecorder(private val context: Context) {
    private val file = File(context.applicationContext.filesDir, "last-crash.txt")

    fun install() {
        val previous = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler { thread, error ->
            runCatching { record(error, fatal = true, thread = thread.name) }
            previous?.uncaughtException(thread, error)
        }
    }

    /** Called for fatal errors (above) and for errors that background work caught and survived. */
    fun record(error: Throwable, fatal: Boolean, thread: String = Thread.currentThread().name) {
        Log.e("VexConnect", "unhandled=$fatal in $thread", error)
        runCatching {
            val version = runCatching { context.packageManager.getPackageInfo(context.packageName, 0).versionName }.getOrNull() ?: "?"
            file.writeText(
                formatCrashReport(fatal, thread, error, version, Build.VERSION.RELEASE, Build.VERSION.SDK_INT, "${Build.MANUFACTURER} ${Build.MODEL}", System.currentTimeMillis()),
            )
        }
    }

    fun read(): String? = runCatching { file.takeIf { it.exists() }?.readText() }.getOrNull()

    fun clear() {
        runCatching { file.delete() }
    }
}

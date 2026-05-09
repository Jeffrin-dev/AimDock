package com.aimdock.helper

import android.accessibilityservice.AccessibilityService
import android.accessibilityservice.GestureDescription
import android.graphics.Path
import android.util.Log
import android.view.accessibility.AccessibilityEvent
import java.io.BufferedReader
import java.io.IOException
import java.io.InputStreamReader
import java.net.ServerSocket
import java.net.Socket
import java.util.Locale
import kotlin.concurrent.thread

class TouchService : AccessibilityService() {
    @Volatile
    private var running = false

    @Volatile
    private var serverSocket: ServerSocket? = null

    @Volatile
    private var activeClient: Socket? = null

    private var serverThread: Thread? = null

    override fun onServiceConnected() {
        super.onServiceConnected()
        startTcpServer()
    }

    override fun onAccessibilityEvent(event: AccessibilityEvent?) = Unit

    override fun onInterrupt() = Unit

    override fun onUnbind(intent: android.content.Intent?): Boolean {
        stopTcpServer()
        return super.onUnbind(intent)
    }

    override fun onDestroy() {
        stopTcpServer()
        super.onDestroy()
    }

    private fun startTcpServer() {
        if (running) return

        running = true
        serverThread = thread(name = "AimDockTouchServer", isDaemon = true) {
            try {
                ServerSocket(SERVER_PORT).use { socket ->
                    serverSocket = socket
                    while (running) {
                        try {
                            socket.accept().use { client ->
                                activeClient = client
                                try {
                                    handleClient(client)
                                } finally {
                                    activeClient = null
                                }
                            }
                        } catch (e: IOException) {
                            if (running) {
                                Log.e(TAG, "Error accepting touch command connection", e)
                            }
                        }
                    }
                }
            } catch (e: IOException) {
                if (running) {
                    Log.e(TAG, "Unable to start touch command server", e)
                }
            } finally {
                serverSocket = null
                running = false
            }
        }
    }

    private fun stopTcpServer() {
        running = false
        try {
            activeClient?.close()
        } catch (e: IOException) {
            Log.w(TAG, "Error closing active touch command connection", e)
        }
        try {
            serverSocket?.close()
        } catch (e: IOException) {
            Log.w(TAG, "Error closing touch command server", e)
        }
        activeClient = null
        serverSocket = null
        serverThread = null
    }

    private fun handleClient(client: Socket) {
        BufferedReader(InputStreamReader(client.getInputStream())).useLines { lines ->
            lines.forEach { line ->
                if (!running) return@forEach
                handleCommand(line.trim())
            }
        }
    }

    private fun handleCommand(command: String) {
        if (command.isBlank()) return

        val parts = command.split(Regex("\\s+"))
        when (parts.firstOrNull()?.uppercase(Locale.US)) {
            "TAP" -> handleTap(parts)
            "SWIPE" -> handleSwipe(parts)
            else -> Log.w(TAG, "Unknown touch command: $command")
        }
    }

    private fun handleTap(parts: List<String>) {
        if (parts.size != 3) {
            Log.w(TAG, "Invalid TAP command")
            return
        }

        val x = parts[1].toFloatOrNull()
        val y = parts[2].toFloatOrNull()
        if (x == null || y == null) {
            Log.w(TAG, "Invalid TAP coordinates")
            return
        }

        val path = Path().apply {
            moveTo(x, y)
        }
        dispatchPath(path, TAP_DURATION_MS)
    }

    private fun handleSwipe(parts: List<String>) {
        if (parts.size != 6) {
            Log.w(TAG, "Invalid SWIPE command")
            return
        }

        val x1 = parts[1].toFloatOrNull()
        val y1 = parts[2].toFloatOrNull()
        val x2 = parts[3].toFloatOrNull()
        val y2 = parts[4].toFloatOrNull()
        val duration = parts[5].toLongOrNull()
        if (x1 == null || y1 == null || x2 == null || y2 == null || duration == null) {
            Log.w(TAG, "Invalid SWIPE arguments")
            return
        }

        val path = Path().apply {
            moveTo(x1, y1)
            lineTo(x2, y2)
        }
        dispatchPath(path, duration)
    }

    private fun dispatchPath(path: Path, durationMs: Long) {
        val gesture = GestureDescription.Builder()
            .addStroke(GestureDescription.StrokeDescription(path, 0, durationMs.coerceAtLeast(1L)))
            .build()
        dispatchGesture(gesture, null, null)
    }

    companion object {
        private const val TAG = "AimDockTouchService"
        private const val SERVER_PORT = 7070
        private const val TAP_DURATION_MS = 50L
    }
}

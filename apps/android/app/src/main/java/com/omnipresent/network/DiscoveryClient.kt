package com.omnipresent.network

import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.InetAddress
import java.net.NetworkInterface
import java.net.SocketTimeoutException

data class DiscoveredServer(val ip: String, val port: Int, val token: Int)

class DiscoveryClient {

    suspend fun discoverServer(timeoutMs: Int = 2500): DiscoveredServer? {
        return withContext(Dispatchers.IO) {
            var socket: DatagramSocket? = null
            try {
                Log.d("UDP_DISCOVERY", "=== Iniciando búsqueda de servidor Omnipresent ===")

                // 1. Obtenemos la mejor dirección de broadcast (Priorizando USB)
                val targetBroadcast = getBestBroadcastAddress()
                    ?: InetAddress.getByName("255.255.255.255").also {
                        Log.w("UDP_DISCOVERY", "Usando fallback global (255.255.255.255) ya que no se encontró una interfaz ideal.")
                    }

                Log.d("UDP_DISCOVERY", "Preparando socket UDP hacia broadcast: ${targetBroadcast.hostAddress}:9091")

                socket = DatagramSocket().apply {
                    broadcast = true
                    soTimeout = timeoutMs
                }

                val sendData = "OMNIPRESENT_DISCOVERY".toByteArray()
                val sendPacket = DatagramPacket(
                    sendData, sendData.size,
                    targetBroadcast, 9091 // Puerto de escucha de descubrimiento en Rust
                )

                Log.d("UDP_DISCOVERY", "Enviando paquete: 'OMNIPRESENT_DISCOVERY' (${sendData.size} bytes)")
                socket.send(sendPacket)

                val recvBuf = ByteArray(1024)
                val recvPacket = DatagramPacket(recvBuf, recvBuf.size)

                Log.d("UDP_DISCOVERY", "Esperando respuesta (Timeout: $timeoutMs ms)...")
                socket.receive(recvPacket)

                val response = String(recvPacket.data, 0, recvPacket.length)
                val senderIp = recvPacket.address.hostAddress
                Log.d("UDP_DISCOVERY", "¡Respuesta recibida desde $senderIp! Contenido: '$response'")

                if (response.startsWith("OMNIPRESENT_HERE")) {
                    val parts = response.split("|")
                    Log.d("UDP_DISCOVERY", "Fragmentos de la respuesta: $parts")

                    // Verificamos que contenga "OMNIPRESENT_HERE", "PUERTO" y "TOKEN"
                    if (parts.size >= 3) {
                        val serverIp = recvPacket.address.hostAddress ?: run {
                            Log.e("UDP_DISCOVERY", "Error: No se pudo obtener la IP del remitente.")
                            return@withContext null
                        }
                        val serverPort = parts[1].toIntOrNull() ?: run {
                            Log.e("UDP_DISCOVERY", "Error: Puerto inválido en la respuesta ('${parts[1]}')")
                            return@withContext null
                        }
                        val token = parts[2].toIntOrNull() ?: run {
                            Log.e("UDP_DISCOVERY", "Error: Token inválido en la respuesta ('${parts[2]}')")
                            return@withContext null
                        }

                        Log.d("UDP_DISCOVERY", "✅ ¡Servidor validado exitosamente! IP: $serverIp, Puerto: $serverPort, Token: $token")
                        return@withContext DiscoveredServer(serverIp, serverPort, token)
                    } else {
                        Log.w("UDP_DISCOVERY", "Respuesta descartada: Faltan datos (esperados 3, recibidos ${parts.size})")
                    }
                } else {
                    Log.w("UDP_DISCOVERY", "Respuesta ignorada: No comienza con 'OMNIPRESENT_HERE'")
                }
            } catch (e: SocketTimeoutException) {
                Log.d("UDP_DISCOVERY", "⏳ Timeout: El tiempo de espera ($timeoutMs ms) expiró sin respuesta del servidor.")
            } catch (e: Exception) {
                Log.e("UDP_DISCOVERY", "❌ Error crítico buscando servidor: ${e.message}", e)
            } finally {
                socket?.close()
                Log.d("UDP_DISCOVERY", "=== Búsqueda finalizada (Socket cerrado) ===")
            }
            null
        }
    }

    /**
     * Revisa las tarjetas de red del teléfono y devuelve la dirección de Broadcast
     * priorizando el cable USB (Tethering) sobre el Wi-Fi.
     */
    private fun getBestBroadcastAddress(): InetAddress? {
        Log.d("UDP_DISCOVERY", "--- Escaneando interfaces de red locales ---")
        var wifiBroadcast: InetAddress? = null
        var usbBroadcast: InetAddress? = null

        try {
            val interfaces = NetworkInterface.getNetworkInterfaces()
            while (interfaces.hasMoreElements()) {
                val networkInterface = interfaces.nextElement()
                val name = networkInterface.name.lowercase()

                // Ignoramos interfaces apagadas o el localhost
                if (networkInterface.isLoopback || !networkInterface.isUp) {
                    Log.v("UDP_DISCOVERY", "Ignorando interfaz: $name (Up: ${networkInterface.isUp}, Loopback: ${networkInterface.isLoopback})")
                    continue
                }

                Log.d("UDP_DISCOVERY", "Analizando interfaz activa: $name")

                for (interfaceAddress in networkInterface.interfaceAddresses) {
                    val broadcast = interfaceAddress.broadcast
                    if (broadcast == null) {
                        Log.v("UDP_DISCOVERY", "  -> Sin dirección broadcast para esta IP en $name")
                        continue
                    }

                    Log.d("UDP_DISCOVERY", "  -> Dirección broadcast encontrada: ${broadcast.hostAddress} en $name")

                    // "rndis" y "usb" son los nombres para USB Tethering en Linux/Android
                    if (name.contains("rndis") || name.contains("usb")) {
                        Log.d("UDP_DISCOVERY", "  ⭐ Interfaz clasificada como USB/Tethering")
                        usbBroadcast = broadcast
                    }
                    // "wlan" es el nombre para la antena Wi-Fi
                    else if (name.contains("wlan")) {
                        Log.d("UDP_DISCOVERY", "  ⭐ Interfaz clasificada como Wi-Fi")
                        wifiBroadcast = broadcast
                    }
                }
            }
        } catch (e: Exception) {
            Log.e("UDP_DISCOVERY", "Error leyendo interfaces de red: ${e.message}", e)
        }

        Log.d("UDP_DISCOVERY", "Resumen de escaneo -> Wi-Fi: ${wifiBroadcast?.hostAddress}, USB: ${usbBroadcast?.hostAddress}")

        // Si encontró el USB, retorna ese. Si no, intenta con el Wi-Fi.
        val selectedAddress = usbBroadcast ?: wifiBroadcast
        Log.d("UDP_DISCOVERY", "--- Interfaz seleccionada para emitir: ${selectedAddress?.hostAddress} ---")

        return selectedAddress
    }
}
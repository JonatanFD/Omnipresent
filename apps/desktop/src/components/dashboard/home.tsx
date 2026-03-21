import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import {
  Play,
  Square,
  RefreshCw,
  Eye,
  EyeOff,
  Smartphone,
  Server,
} from "lucide-react";
import { QRCode } from "../kibo-ui/qr-code";
import { GithubDark } from "../ui/svgs/githubDark";
import { GithubLight } from "../ui/svgs/githubLight";
import { Link } from "react-router-dom"; // Mantenido por si lo usas en otro lado
import Kofi from "../ui/svgs/kofi";

export function Home() {
  const [isRunning, setIsRunning] = useState(false);
  const [showQR, setShowQR] = useState(true);
  const [port, setPort] = useState("1420");
  const [devices, setDevices] = useState([
    { id: 1, name: "iPhone de Juan", ip: "192.168.1.45" },
    { id: 2, name: "Samsung Galaxy S23", ip: "192.168.1.102" },
  ]);

  const toggleServer = () => setIsRunning(!isRunning);
  const toggleQR = () => setShowQR(!showQR);
  const generateNewQR = () => console.log("Generando nuevo QR...");

  return (
    // CONTENEDOR PRINCIPAL: Usamos Grid. 1 columna en móvil, 2 columnas desde 'md'
    <main className="p-6 max-w-5xl mx-auto grid grid-cols-1 md:grid-cols-2 gap-6">
      {/* SECCIÓN 1: Controles del Servidor */}
      <Card className="col-span-1 flex flex-col">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Server className="w-5 h-5" />
            Server Management
          </CardTitle>
          <CardDescription>Configura el servidor local</CardDescription>
        </CardHeader>
        {/* Usamos flex-col con un gap uniforme para que los elementos respiren */}
        <CardContent className="flex flex-col gap-6 flex-grow">
          <div className="flex items-center justify-between p-4 bg-muted/40 rounded-lg border">
            <div className="space-y-0.5">
              <p className="text-sm font-medium">Estado del Servicio</p>
              <p className="text-xs text-muted-foreground">
                {isRunning ? "Ejecutándose en la red local" : "Apagado"}
              </p>
            </div>
            <Button
              onClick={toggleServer}
              variant={isRunning ? "destructive" : "default"}
              size="sm"
            >
              {isRunning ? (
                <>
                  <Square className="w-4 h-4 mr-2" /> Stop
                </>
              ) : (
                <>
                  <Play className="w-4 h-4 mr-2" /> Start
                </>
              )}
            </Button>
          </div>

          <div className="space-y-3">
            <Label htmlFor="port">Puerto del Servicio</Label>
            <Input
              id="port"
              value={port}
              onChange={(e) => setPort(e.target.value)}
              disabled={isRunning}
              placeholder="Ej. 8080"
            />
            <p className="text-xs text-muted-foreground">
              Debes detener el servidor para cambiar el puerto.
            </p>
          </div>
        </CardContent>
      </Card>

      {/* SECCIÓN 2: Código QR */}
      <Card className="col-span-1 flex flex-col">
        <CardHeader>
          <CardTitle>Conexión Rápida</CardTitle>
          <CardDescription>
            Escanea el código para conectar un dispositivo.
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col items-center justify-between flex-grow space-y-6">
          <div className="w-48 h-48 bg-muted rounded-lg border-2 border-dashed flex items-center justify-center relative overflow-hidden">
            {showQR ? (
              <QRCode
                className="size-48 rounded border bg-white p-4 shadow-xs"
                data={`omnipresent://${192}:${port}`} // Idealmente aquí va la IP real
              />
            ) : (
              <div className="text-muted-foreground flex flex-col items-center">
                <EyeOff className="w-8 h-8 mb-2 opacity-50" />
                <span className="text-sm">QR Oculto</span>
              </div>
            )}
          </div>

          <div className="flex gap-3 w-full justify-center">
            <Button
              variant="outline"
              onClick={toggleQR}
              className="w-full max-w-[140px]"
            >
              {showQR ? (
                <EyeOff className="w-4 h-4 mr-2" />
              ) : (
                <Eye className="w-4 h-4 mr-2" />
              )}
              {showQR ? "Ocultar" : "Mostrar"}
            </Button>
            <Button
              variant="secondary"
              onClick={generateNewQR}
              className="w-full max-w-[140px]"
            >
              <RefreshCw className="w-4 h-4 mr-2" />
              Nuevo QR
            </Button>
          </div>
        </CardContent>
      </Card>

      {/* SECCIÓN 3: Lista de Dispositivos (Ocupa ambas columnas en PC) */}
      <Card className="col-span-1 md:col-span-2">
        <CardHeader>
          <CardTitle>Dispositivos Conectados</CardTitle>
          <CardDescription>
            Administra los clientes enlazados a tu servidor.
          </CardDescription>
        </CardHeader>
        <CardContent>
          {devices.length > 0 ? (
            <div className="grid gap-3 sm:grid-cols-2">
              {/* Convertimos la lista en otro grid interno para aprovechar mejor el ancho en pantallas grandes */}
              {devices.map((device) => (
                <div
                  key={device.id}
                  className="flex items-center justify-between p-4 border rounded-lg bg-card hover:bg-muted/40 transition-colors"
                >
                  <div className="flex items-center gap-3">
                    <div className="p-2 bg-primary/10 rounded-full">
                      <Smartphone className="w-4 h-4 text-primary" />
                    </div>
                    <div>
                      <p className="text-sm font-medium">{device.name}</p>
                      <p className="text-xs text-muted-foreground">
                        {device.ip}
                      </p>
                    </div>
                  </div>
                  <Badge
                    variant="outline"
                    className="bg-green-50 text-green-700 border-green-200 dark:bg-green-950 dark:text-green-400 dark:border-green-900"
                  >
                    Conectado
                  </Badge>
                </div>
              ))}
            </div>
          ) : (
            <div className="py-12 text-center border-2 border-dashed rounded-lg">
              <Smartphone className="w-8 h-8 mx-auto text-muted-foreground mb-3 opacity-50" />
              <p className="text-sm text-muted-foreground">
                No hay dispositivos conectados actualmente.
              </p>
            </div>
          )}
        </CardContent>
      </Card>

      {/* FOOTER: Botones Sociales (Ocupa ambas columnas, centrado) */}
      <div className="col-span-1 md:col-span-2 flex flex-wrap items-center justify-center gap-4 pt-4 border-t">
        <a href="https://ko-fi.com/U7U51VV3PC" target="_blank" rel="noreferrer">
          <Button variant="outline">
            <Kofi className="size-4" />
            Buy me a coffee
          </Button>
        </a>
        <Button variant="outline">
          <GithubDark className="hidden dark:inline w-4 h-4 mr-2" />
          <GithubLight className="dark:hidden w-4 h-4 mr-2" />
          Star on GitHub
        </Button>
      </div>
    </main>
  );
}

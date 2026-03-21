import { useCallback, useEffect, useMemo, useState } from "react";
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
  Wifi,
  WifiOff,
  Server,
} from "lucide-react";
import { QRCode } from "../kibo-ui/qr-code";
import { GithubDark } from "../ui/svgs/githubDark";
import { GithubLight } from "../ui/svgs/githubLight";
import Kofi from "../ui/svgs/kofi";
import {
  getCoreStatus,
  getCurrentWifiInfo,
  startCoreService,
  stopCoreService,
} from "@/lib/tauri-api";
import type { CoreServiceStatus, WifiInfo } from "@/lib/types";

const DEFAULT_PORT = 9090;

export function Home() {
  const [showQR, setShowQR] = useState(true);
  const [port, setPort] = useState(String(DEFAULT_PORT));
  const [status, setStatus] = useState<CoreServiceStatus>({
    running: false,
    connection: null,
  });
  const [wifiInfo, setWifiInfo] = useState<WifiInfo>({
    connected: false,
    ssid: null,
    interface: null,
  });
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const hydrateStatus = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      const [coreStatus, wifi] = await Promise.all([
        getCoreStatus(),
        getCurrentWifiInfo(),
      ]);
      setStatus(coreStatus);
      setWifiInfo(wifi);

      if (coreStatus.connection?.port) {
        setPort(String(coreStatus.connection.port));
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load app state");
    } finally {
      setIsLoading(false);
    }
  }, []);

  useEffect(() => {
    hydrateStatus();
  }, [hydrateStatus]);

  const toggleServer = useCallback(async () => {
    setError(null);
    try {
      if (status.running) {
        const next = await stopCoreService();
        setStatus(next);
        return;
      }

      const parsedPort = Number.parseInt(port, 10);
      if (Number.isNaN(parsedPort) || parsedPort < 1 || parsedPort > 65535) {
        setError("Port must be between 1 and 65535");
        return;
      }

      const next = await startCoreService(parsedPort, false);
      setStatus(next);
      const wifi = await getCurrentWifiInfo();
      setWifiInfo(wifi);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Service action failed");
    }
  }, [port, status.running]);

  const generateNewQR = useCallback(async () => {
    if (!status.running || !status.connection) {
      setError("Start the service before generating a new QR");
      return;
    }

    setError(null);
    try {
      const next = await startCoreService(status.connection.port, true);
      setStatus(next);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to refresh QR data");
    }
  }, [status.connection, status.running]);

  const connectionLabel = useMemo(() => {
    if (!status.running || !status.connection) {
      return "Service stopped";
    }

    const ip = status.connection.ip ?? "0.0.0.0";
    return `${ip}:${status.connection.port}`;
  }, [status.connection, status.running]);

  return (
    <main className="p-6 max-w-5xl mx-auto grid grid-cols-1 md:grid-cols-2 gap-6">
      <Card className="col-span-1 flex flex-col">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Server className="w-5 h-5" />
            Core Service
          </CardTitle>
          <CardDescription>
            Orquesta el backend modular desde Tauri
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-6 flex-grow">
          <div className="flex items-center justify-between p-4 bg-muted/40 rounded-lg border">
            <div className="space-y-0.5">
              <p className="text-sm font-medium">Estado del servicio</p>
              <p className="text-xs text-muted-foreground">{connectionLabel}</p>
            </div>
            <Button
              onClick={toggleServer}
              variant={status.running ? "destructive" : "default"}
              size="sm"
              disabled={isLoading}
            >
              {status.running ? (
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
            <Label htmlFor="port">Puerto del servicio</Label>
            <Input
              id="port"
              value={port}
              onChange={(event) => setPort(event.target.value)}
              disabled={status.running}
              placeholder="9090"
            />
            <p className="text-xs text-muted-foreground">
              Deten el servicio para cambiar el puerto.
            </p>
          </div>

          <div className="p-4 rounded-lg border bg-muted/40">
            <p className="text-sm font-medium mb-1">Token actual</p>
            <p className="text-xs text-muted-foreground">
              {status.connection?.token ?? "Unavailable"}
            </p>
          </div>
        </CardContent>
      </Card>

      <Card className="col-span-1 flex flex-col">
        <CardHeader>
          <CardTitle>Conexión rápida</CardTitle>
          <CardDescription>
            Escanea el QR para conectar un dispositivo móvil
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col items-center justify-between flex-grow space-y-6">
          <div className="w-48 h-48 bg-muted rounded-lg border-2 border-dashed flex items-center justify-center relative overflow-hidden">
            {showQR && status.connection?.qr_payload ? (
              <QRCode
                className="size-48 rounded border bg-white p-4 shadow-xs"
                data={status.connection.qr_payload}
              />
            ) : (
              <div className="text-muted-foreground flex flex-col items-center">
                <EyeOff className="w-8 h-8 mb-2 opacity-50" />
                <span className="text-sm">
                  {status.running ? "QR hidden" : "Start service to render QR"}
                </span>
              </div>
            )}
          </div>

          <div className="flex gap-3 w-full justify-center">
            <Button
              variant="outline"
              onClick={() => setShowQR((value) => !value)}
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
              disabled={!status.running}
            >
              <RefreshCw className="w-4 h-4 mr-2" />
              Nuevo QR
            </Button>
          </div>
        </CardContent>
      </Card>

      <Card className="col-span-1 md:col-span-2">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            {wifiInfo.connected ? (
              <Wifi className="w-5 h-5" />
            ) : (
              <WifiOff className="w-5 h-5" />
            )}
            WiFi actual
          </CardTitle>
          <CardDescription>
            Endpoint del backend con la red activa de la PC
          </CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-4">
          <div className="flex flex-wrap items-center gap-3">
            <Badge variant={wifiInfo.connected ? "default" : "outline"}>
              {wifiInfo.connected ? "Connected" : "Disconnected"}
            </Badge>
            <span className="text-sm text-muted-foreground">
              SSID: {wifiInfo.ssid ?? "N/A"}
            </span>
            <span className="text-sm text-muted-foreground">
              Interface: {wifiInfo.interface ?? "N/A"}
            </span>
          </div>

          <div>
            <Button variant="outline" size="sm" onClick={hydrateStatus}>
              <RefreshCw className="w-4 h-4 mr-2" />
              Refresh network status
            </Button>
          </div>

          {error ? (
            <p className="text-sm text-destructive">{error}</p>
          ) : null}
        </CardContent>
      </Card>

      <div className="col-span-1 md:col-span-2 flex flex-wrap items-center justify-center gap-4 pt-4 border-t">
        <a href="https://ko-fi.com/U7U51VV3PC" target="_blank" rel="noreferrer">
          <Button className="bg-[#72A5F2] border-none hover:bg-[#72A5F2]/90">
            <Kofi className="size-4" />
            Support me on Ko-fi
          </Button>
        </a>
        <a
          href="https://github.com/JonatanFD/Omnipresent"
          target="_blank"
          rel="noreferrer"
        >
          <Button variant="outline">
            <GithubDark className="hidden dark:inline w-4 h-4 mr-2" />
            <GithubLight className="dark:hidden w-4 h-4 mr-2" />
            Star on GitHub
          </Button>
        </a>
      </div>
    </main>
  );
}

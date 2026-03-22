import { useCallback, useEffect, useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Separator } from "@/components/ui/separator";
import { Badge } from "@/components/ui/badge";
import {
  Play,
  Square,
  RefreshCw,
  Eye,
  EyeOff,
  Wifi,
  WifiOff,
  Copy,
  Check,
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
  const [copied, setCopied] = useState(false);

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
      setError(
        err instanceof Error ? err.message : "Failed to refresh QR data",
      );
    }
  }, [status.connection, status.running]);

  const connectionLabel = useMemo(() => {
    if (!status.running || !status.connection) return null;
    const ip = status.connection.ip ?? "0.0.0.0";
    return `${ip}:${status.connection.port}`;
  }, [status.connection, status.running]);

  const copyToken = useCallback(() => {
    if (!status.connection?.token) return;
    navigator.clipboard.writeText(status.connection.token.toString());
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }, [status.connection?.token]);

  return (
    <main className="min-h-screen bg-background">
      {/* Header */}
      <div className="border-b px-6 py-4 flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div
            className={`w-2 h-2 rounded-full transition-colors ${
              status.running ? "bg-green-500" : "bg-muted-foreground/40"
            }`}
          />
          <span className="text-sm font-medium">Omnipresent</span>
          {connectionLabel && (
            <span className="text-xs text-muted-foreground font-mono">
              {connectionLabel}
            </span>
          )}
        </div>
        <div className="flex items-center gap-2 text-xs text-muted-foreground">
          {wifiInfo.connected ? (
            <Wifi className="w-3.5 h-3.5" />
          ) : (
            <WifiOff className="w-3.5 h-3.5" />
          )}
          <span>{wifiInfo.ssid ?? "No network"}</span>
        </div>
      </div>

      {/* Body */}
      <div className="max-w-lg mx-auto px-6 py-10 space-y-8">
        {/* Service control */}
        <section className="space-y-4">
          <div className="flex items-center justify-between">
            <h2 className="text-sm font-medium text-foreground">
              Core service
            </h2>
            <Badge
              variant={status.running ? "default" : "secondary"}
              className="text-xs"
            >
              {status.running ? "Running" : "Stopped"}
            </Badge>
          </div>

          <div className="flex gap-2">
            <div className="flex-1 space-y-1.5">
              <Label htmlFor="port" className="text-xs text-muted-foreground">
                Port
              </Label>
              <Input
                id="port"
                value={port}
                onChange={(e) => setPort(e.target.value)}
                disabled={status.running}
                placeholder="9090"
                className="font-mono text-sm h-9"
              />
            </div>
            <div className="flex items-end">
              <Button
                onClick={toggleServer}
                variant={status.running ? "destructive" : "default"}
                size="sm"
                disabled={isLoading}
                className="h-9 px-4"
              >
                {status.running ? (
                  <>
                    <Square className="w-3.5 h-3.5 mr-1.5" />
                    Stop
                  </>
                ) : (
                  <>
                    <Play className="w-3.5 h-3.5 mr-1.5" />
                    Start
                  </>
                )}
              </Button>
            </div>
          </div>

          {error && (
            <p className="text-xs text-destructive bg-destructive/10 rounded px-3 py-2">
              {error}
            </p>
          )}
        </section>

        <Separator />

        {/* Token */}
        <section className="space-y-3">
          <h2 className="text-sm font-medium text-foreground">Session token</h2>
          <div className="flex items-center gap-2 p-3 rounded-md bg-muted/50 border">
            <p className="flex-1 text-xs font-mono text-muted-foreground truncate">
              {status.connection?.token ?? "—"}
            </p>
            <Button
              variant="ghost"
              size="icon"
              className="h-6 w-6 shrink-0"
              disabled={!status.connection?.token}
              onClick={copyToken}
            >
              {copied ? (
                <Check className="w-3.5 h-3.5 text-green-500" />
              ) : (
                <Copy className="w-3.5 h-3.5" />
              )}
            </Button>
          </div>
        </section>

        <Separator />

        {/* QR code */}
        <section className="space-y-4">
          <div className="flex items-center justify-between">
            <h2 className="text-sm font-medium text-foreground">
              Quick connect
            </h2>
            <div className="flex gap-2">
              <Button
                variant="ghost"
                size="sm"
                className="h-7 text-xs gap-1.5"
                onClick={() => setShowQR((v) => !v)}
              >
                {showQR ? (
                  <EyeOff className="w-3.5 h-3.5" />
                ) : (
                  <Eye className="w-3.5 h-3.5" />
                )}
                {showQR ? "Hide" : "Show"}
              </Button>
              <Button
                variant="ghost"
                size="sm"
                className="h-7 text-xs gap-1.5"
                disabled={!status.running}
                onClick={generateNewQR}
              >
                <RefreshCw className="w-3.5 h-3.5" />
                Refresh
              </Button>
            </div>
          </div>

          <div className="flex justify-center py-4">
            {showQR && status.connection?.qr_payload ? (
              <div className="rounded-xl border bg-white p-4 shadow-sm">
                <QRCode
                  className="size-44"
                  data={status.connection.qr_payload}
                />
              </div>
            ) : (
              <div className="size-52 rounded-xl border-2 border-dashed flex flex-col items-center justify-center text-muted-foreground gap-2">
                <EyeOff className="w-6 h-6 opacity-30" />
                <span className="text-xs">
                  {status.running
                    ? "QR code hidden"
                    : "Start service to show QR"}
                </span>
              </div>
            )}
          </div>
        </section>

        <Separator />

        {/* Network info & footer */}
        <section className="space-y-4">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-2">
              {wifiInfo.connected ? (
                <Wifi className="w-4 h-4 text-muted-foreground" />
              ) : (
                <WifiOff className="w-4 h-4 text-muted-foreground" />
              )}
              <span className="text-sm font-medium">
                {wifiInfo.ssid ?? "No network"}
              </span>
              {wifiInfo.interface && (
                <span className="text-xs text-muted-foreground font-mono">
                  {wifiInfo.interface}
                </span>
              )}
            </div>
            <Button
              variant="ghost"
              size="sm"
              className="h-7 text-xs gap-1.5"
              onClick={hydrateStatus}
            >
              <RefreshCw className="w-3.5 h-3.5" />
              Refresh
            </Button>
          </div>
        </section>

        <Separator />

        {/* Links */}
        <footer className="flex items-center justify-center gap-3 pt-2">
          <a
            href="https://ko-fi.com/U7U51VV3PC"
            target="_blank"
            rel="noreferrer"
          >
            <Button
              size="sm"
              className="h-8 text-xs bg-[#72A5F2] hover:bg-[#72A5F2]/90 border-none gap-1.5"
            >
              <Kofi className="size-3.5" />
              Ko-fi
            </Button>
          </a>
          <a
            href="https://github.com/JonatanFD/Omnipresent"
            target="_blank"
            rel="noreferrer"
          >
            <Button variant="outline" size="sm" className="h-8 text-xs gap-1.5">
              <GithubDark className="hidden dark:inline w-3.5 h-3.5" />
              <GithubLight className="dark:hidden w-3.5 h-3.5" />
              GitHub
            </Button>
          </a>
        </footer>
      </div>
    </main>
  );
}

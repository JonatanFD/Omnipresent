using System.Collections.ObjectModel;
using System.Globalization;
using Omni.Ipc;

namespace Omni.App.Core;

/// <summary>
/// The app's main view model: a live, bindable view of the daemon. It performs the
/// version handshake, follows the daemon's push stream to keep state current,
/// reconnects on its own when the daemon goes away, and exposes commands that map
/// straight to IPC requests. It holds no business logic — the daemon owns state.
///
/// Every piece of state the UI needs is computed here rather than in a view, so
/// the panes stay thin bindings and the arrangement can be verified by tests.
/// Mirrors the macOS client's <c>DaemonViewModel</c>.
/// </summary>
public sealed class DaemonViewModel : ObservableObject
{
    private readonly IOmniDaemonClient _client;
    private readonly IUiDispatcher _ui;
    private readonly TimeSpan _reconnectDelay;
    private readonly OmniBinaryLocator _binaries;

    /// <summary>Guards the run-loop handles, which the UI thread and the loop share.</summary>
    private readonly Lock _lifecycle = new();
    private CancellationTokenSource? _loopCts;
    private Task? _loop;
    private CancellationToken _appToken = CancellationToken.None;

    public DaemonViewModel(
        IOmniDaemonClient client,
        IUiDispatcher ui,
        TimeSpan? reconnectDelay = null,
        OmniBinaryLocator? binaries = null)
    {
        _client = client;
        _ui = ui;
        _reconnectDelay = reconnectDelay ?? TimeSpan.FromSeconds(2);
        _binaries = binaries ?? OmniBinaryLocator.Default;

        // Keep the derived collection flags in step with the collections; these
        // run on the UI thread, inside the same update.
        Pending.CollectionChanged += (_, _) =>
        {
            Raise(nameof(HasPending));
            Raise(nameof(PendingCount));
            RaiseConnectionsVisibility();
        };
        Sessions.CollectionChanged += (_, _) =>
        {
            Raise(nameof(HasSessions));
            RaiseConnectionsVisibility();
        };
        Peers.CollectionChanged += (_, _) =>
        {
            Raise(nameof(HasPeers));
            RaiseConnectionsVisibility();
        };
        Placements.CollectionChanged += (_, _) =>
        {
            Raise(nameof(HasPlacements));
            RaiseConnectionsVisibility();
        };
    }

    private ConnectionStatus _connection = ConnectionStatus.Connecting;
    public ConnectionStatus Connection
    {
        get => _connection;
        private set
        {
            if (SetField(ref _connection, value))
            {
                Raise(nameof(IsConnected));
                Raise(nameof(IsIncompatible));
                Raise(nameof(CanStartDaemon));
                Raise(nameof(CanStopDaemon));
                Raise(nameof(ShowDaemonSections));
                RaiseConnectionsVisibility();
            }
        }
    }

    /// <summary>True only while live, for enabling/disabling controls.</summary>
    public bool IsConnected => _connection == ConnectionStatus.Connected;

    /// <summary>True when the daemon is too new; the UI shows an "update" notice.</summary>
    public bool IsIncompatible => _connection == ConnectionStatus.Incompatible;

    /// <summary>
    /// Whether the General pane shows the daemon controls at all. An incompatible
    /// daemon replaces them with the update notice, as on macOS.
    /// </summary>
    public bool ShowDaemonSections => !IsIncompatible;

    /// <summary>Starting only makes sense when the daemon is down and usable.</summary>
    public bool CanStartDaemon => !IsConnected && !IsIncompatible;

    /// <summary>Stopping only makes sense while there is a daemon to stop.</summary>
    public bool CanStopDaemon => IsConnected;

    private string _statusText = "Connecting…";
    public string StatusText { get => _statusText; private set => SetField(ref _statusText, value); }

    private string _fingerprint = "";
    public string Fingerprint { get => _fingerprint; private set => SetField(ref _fingerprint, value); }

    private int _port;
    public int Port
    {
        get => _port;
        private set
        {
            if (SetField(ref _port, value))
            {
                Raise(nameof(PortText));
            }
        }
    }

    /// <summary>The port as text, for the Info row.</summary>
    public string PortText => _port.ToString(CultureInfo.InvariantCulture);

    private bool _capturing;
    public bool Capturing
    {
        get => _capturing;
        private set
        {
            if (SetField(ref _capturing, value))
            {
                Raise(nameof(CaptureStatus));
            }
        }
    }

    /// <summary>How the Info pane words the capture state, as on macOS.</summary>
    public string CaptureStatus => _capturing ? "Active" : "Target only";

    private bool _clipboardSharing;
    public bool ClipboardSharing { get => _clipboardSharing; private set => SetField(ref _clipboardSharing, value); }

    private string _daemonVersion = "";
    public string DaemonVersion
    {
        get => _daemonVersion;
        private set
        {
            if (SetField(ref _daemonVersion, value))
            {
                Raise(nameof(DisplayVersion));
                Raise(nameof(HasDaemonVersion));
            }
        }
    }

    /// <summary>The version with its "v" prefix, or empty while unknown.</summary>
    public string DisplayVersion => string.IsNullOrEmpty(_daemonVersion) ? "" : $"v{_daemonVersion}";

    /// <summary>Whether a version is known yet, so the row can be hidden.</summary>
    public bool HasDaemonVersion => !string.IsNullOrEmpty(_daemonVersion);

    private string? _lastError;
    public string? LastError
    {
        get => _lastError;
        private set
        {
            if (SetField(ref _lastError, value))
            {
                Raise(nameof(HasError));
            }
        }
    }

    /// <summary>True when there is an error message to show.</summary>
    public bool HasError => !string.IsNullOrEmpty(_lastError);

    public ObservableCollection<SessionInfo> Sessions { get; } = [];
    public ObservableCollection<PendingInfo> Pending { get; } = [];
    public ObservableCollection<PeerInfo> Peers { get; } = [];
    public ObservableCollection<LayoutInfo> Placements { get; } = [];

    public bool HasPending => Pending.Count > 0;
    public bool HasSessions => Sessions.Count > 0;
    public bool HasPeers => Peers.Count > 0;
    public bool HasPlacements => Placements.Count > 0;

    /// <summary>How many requests are waiting, for the navigation badge.</summary>
    public int PendingCount => Pending.Count;

    /// <summary>Whether the Connections pane knows anything worth showing.</summary>
    public bool HasConnectionsData => HasSessions || HasPending || HasPeers || HasPlacements;

    /// <summary>
    /// The Connections pane falls back to a centred "Daemon Not Running" message
    /// when there is no daemon and nothing remembered, exactly as on macOS.
    /// </summary>
    public bool ShowConnectionsEmptyState => !IsConnected && !HasConnectionsData;

    /// <summary>The inverse of <see cref="ShowConnectionsEmptyState"/>.</summary>
    public bool ShowConnectionsContent => !ShowConnectionsEmptyState;

    // -----------------------------------------------------------------------
    // Run loop
    // -----------------------------------------------------------------------

    /// <summary>
    /// Starts the connect/subscribe/reconnect loop once, for the whole app
    /// lifetime. Idempotent: safe to call from more than one view.
    /// </summary>
    public void Start(CancellationToken appToken)
    {
        lock (_lifecycle)
        {
            _appToken = appToken;
            if (_loop is not null)
            {
                return;
            }
            _loop = StartLoop();
        }
    }

    /// <summary>
    /// Cancels the current loop and starts a fresh one at once, skipping any
    /// reconnect delay it was sleeping through. This is what makes the UI go live
    /// immediately after <see cref="StartDaemonAsync"/>.
    /// </summary>
    public void ReconnectNow()
    {
        lock (_lifecycle)
        {
            _loopCts?.Cancel();
            _loop = StartLoop();
        }
    }

    /// <summary>Stops the loop and waits for it to unwind.</summary>
    public async Task ShutdownAsync()
    {
        Task? loop;
        lock (_lifecycle)
        {
            _loopCts?.Cancel();
            loop = _loop;
            _loop = null;
        }
        if (loop is not null)
        {
            await loop.ConfigureAwait(false);
        }
    }

    /// <summary>
    /// Spawns a loop on a fresh linked token, and disposes that token source once
    /// the loop has unwound (never before — the loop may still be observing it).
    /// </summary>
    private Task StartLoop()
    {
        var cts = CancellationTokenSource.CreateLinkedTokenSource(_appToken);
        _loopCts = cts;
        return Task.Run(() => RunAsync(cts.Token))
            .ContinueWith(_ => cts.Dispose(), TaskScheduler.Default);
    }

    /// <summary>
    /// Runs until <paramref name="cancellationToken"/> is cancelled: handshake,
    /// then follow the push stream, reconnecting after a delay whenever it drops.
    /// Stops permanently only if the daemon is too new (see
    /// <see cref="ConnectionStatus.Incompatible"/>).
    /// </summary>
    public async Task RunAsync(CancellationToken cancellationToken)
    {
        while (!cancellationToken.IsCancellationRequested)
        {
            try
            {
                var hello = await _client.HelloAsync(cancellationToken).ConfigureAwait(false);
                if (hello.ProtocolVersion > OmniProtocol.Version)
                {
                    SetState(ConnectionStatus.Incompatible,
                        $"The daemon speaks protocol v{hello.ProtocolVersion}; this app understands v{OmniProtocol.Version}. Please update Omnipresent.");
                    return;
                }
                _ui.Post(() => DaemonVersion = hello.DaemonVersion);

                await foreach (var snapshot in _client.SubscribeAsync(cancellationToken).ConfigureAwait(false))
                {
                    Apply(snapshot);
                    await RefreshListsAsync(cancellationToken).ConfigureAwait(false);
                    SetState(ConnectionStatus.Connected, "Connected");
                }
            }
            catch (OperationCanceledException)
            {
                break;
            }
            catch (OmniDaemonException ex)
            {
                SetState(ConnectionStatus.Disconnected, ex.Message);
            }
            catch (Exception ex)
            {
                // Anything else — a malformed line, an unknown event from a newer
                // daemon, a pipe fault — must not end the loop, or the window
                // freezes on stale state with no way back. Report and retry.
                SetState(ConnectionStatus.Disconnected, ex.Message);
            }

            if (cancellationToken.IsCancellationRequested)
            {
                break;
            }
            // Amber, not red, while waiting: the daemon may simply be starting up.
            SetState(ConnectionStatus.Connecting, "Waiting for the daemon…");
            try
            {
                await Task.Delay(_reconnectDelay, cancellationToken).ConfigureAwait(false);
            }
            catch (OperationCanceledException)
            {
                break;
            }
        }
    }

    // -----------------------------------------------------------------------
    // Commands
    // -----------------------------------------------------------------------

    public Task ConnectAsync(string host) => Guard(ct => _client.ConnectAsync(host, ct));
    public Task DisconnectAsync(string host) => Guard(ct => _client.DisconnectAsync(host, ct));
    public Task AcceptAsync(string selector) => Guard(ct => _client.AcceptAsync(selector, ct));
    public Task RejectAsync(string selector) => Guard(ct => _client.RejectAsync(selector, ct));
    public Task RemovePeerAsync(string selector) => Guard(ct => _client.RemovePeerAsync(selector, ct));
    public Task SetLayoutAsync(string host, string edge) => Guard(ct => _client.SetLayoutAsync(host, edge, ct));
    public Task SetClipboardAsync(bool enabled) => Guard(ct => _client.SetClipboardAsync(enabled, ct));
    public Task StopDaemonAsync() => Guard(ct => _client.StopAsync(ct));

    /// <summary>
    /// Launches <c>omni start</c>, then retries the connection at once so the UI
    /// goes live without waiting for the next scheduled reconnect.
    /// </summary>
    public async Task StartDaemonAsync()
    {
        _ui.Post(() => LastError = null);
        if (_binaries.Locate() is not { } binaryPath)
        {
            _ui.Post(() => LastError = OmniBinaryLocator.NotFoundMessage());
            return;
        }
        try
        {
            using var process = new System.Diagnostics.Process
            {
                StartInfo = new System.Diagnostics.ProcessStartInfo
                {
                    FileName = binaryPath,
                    Arguments = "start",
                    UseShellExecute = false,
                    CreateNoWindow = true,
                }
            };
            process.Start();
        }
        catch (Exception ex)
        {
            _ui.Post(() => LastError = $"Failed to start the daemon: {ex.Message}");
            return;
        }
        await Task.Delay(500).ConfigureAwait(false);
        ReconnectNow();
    }

    /// <summary>
    /// Runs <c>omni update</c> to completion and reports what happened. Lives here
    /// rather than in the Update pane so it is testable and matches macOS, where
    /// the same work sits behind the view model.
    /// </summary>
    public async Task<string> RunUpdateAsync()
    {
        if (_binaries.Locate() is not { } binaryPath)
        {
            return OmniBinaryLocator.NotFoundMessage();
        }
        try
        {
            using var process = new System.Diagnostics.Process
            {
                StartInfo = new System.Diagnostics.ProcessStartInfo
                {
                    FileName = binaryPath,
                    Arguments = "update",
                    UseShellExecute = false,
                    CreateNoWindow = true,
                }
            };
            process.Start();
            await process.WaitForExitAsync().ConfigureAwait(false);
            return process.ExitCode == 0
                ? "Update complete. The daemon will restart shortly."
                : $"Update exited with code {process.ExitCode}.";
        }
        catch (Exception ex)
        {
            return $"Failed to run update: {ex.Message}";
        }
    }

    // -----------------------------------------------------------------------
    // Internals
    // -----------------------------------------------------------------------

    private async Task Guard(Func<CancellationToken, Task> action)
    {
        _ui.Post(() => LastError = null);
        try
        {
            await action(CancellationToken.None).ConfigureAwait(false);
        }
        catch (OmniDaemonException ex)
        {
            _ui.Post(() => LastError = ex.Message);
        }
    }

    private void Apply(StatusInfo snapshot) => _ui.Post(() =>
    {
        Fingerprint = snapshot.Fingerprint;
        Port = snapshot.Port;
        Capturing = snapshot.Capturing;
        ClipboardSharing = snapshot.ClipboardSharing;
        Replace(Sessions, snapshot.Sessions);
        Replace(Pending, snapshot.Pending);
    });

    private async Task RefreshListsAsync(CancellationToken cancellationToken)
    {
        // Peers and placements are separate requests, not part of the snapshot;
        // refresh them whenever state changes so the lists stay live.
        try
        {
            var peers = await _client.PeersAsync(cancellationToken).ConfigureAwait(false);
            var placements = await _client.LayoutAsync(cancellationToken).ConfigureAwait(false);
            _ui.Post(() =>
            {
                Replace(Peers, peers);
                Replace(Placements, placements);
            });
        }
        catch (OmniDaemonException)
        {
            // Keep whatever we last had; the snapshot itself still applied.
        }
    }

    private void SetState(ConnectionStatus status, string text) => _ui.Post(() =>
    {
        Connection = status;
        StatusText = text;
        if (status == ConnectionStatus.Incompatible)
        {
            LastError = text;
        }
    });

    /// <summary>Raises the flags that depend on both the connection and the data.</summary>
    private void RaiseConnectionsVisibility()
    {
        Raise(nameof(HasConnectionsData));
        Raise(nameof(ShowConnectionsEmptyState));
        Raise(nameof(ShowConnectionsContent));
    }

    private static void Replace<T>(ObservableCollection<T> target, IReadOnlyList<T> items)
    {
        target.Clear();
        foreach (var item in items)
        {
            target.Add(item);
        }
    }
}

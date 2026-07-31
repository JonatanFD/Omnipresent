using Omni.App.Core;
using Omni.Ipc;

namespace Omni.App.Tests;

public class DaemonViewModelTests
{
    private static StatusInfo Snapshot(bool capturing = false, bool clipboard = false) =>
        new("ab".PadRight(4, 'c'), 4733, capturing, clipboard,
            [new SessionInfo("mac", "cd", "controller", true)],
            [new PendingInfo("win", "ef")]);

    private static DaemonViewModel NewViewModel(FakeDaemonClient client) =>
        new(client, new ImmediateDispatcher(), reconnectDelay: TimeSpan.FromMinutes(5));

    [Fact]
    public async Task Run_applies_pushed_snapshots_and_goes_connected()
    {
        var client = new FakeDaemonClient();
        client.Snapshots.Add(Snapshot(capturing: false));
        client.Snapshots.Add(Snapshot(capturing: true));
        client.PeersList.Add(new PeerInfo("mac", "cd", true));
        client.Placements.Add(new LayoutInfo("mac", "left", true));

        using var cts = new CancellationTokenSource();
        // Stop the reconnect loop once the stream has been fully consumed.
        client.OnSubscribeDrained = cts.Cancel;

        var vm = NewViewModel(client);
        await vm.RunAsync(cts.Token);

        Assert.Equal(ConnectionStatus.Connected, vm.Connection);
        Assert.True(vm.IsConnected);
        Assert.Equal(4733, vm.Port);
        Assert.True(vm.Capturing); // from the last snapshot
        Assert.Equal("0.3.7", vm.DaemonVersion);
        Assert.Equal("mac", Assert.Single(vm.Sessions).Host);
        Assert.Equal("win", Assert.Single(vm.Pending).Host);
        Assert.Equal("mac", Assert.Single(vm.Peers).Host);
        Assert.Equal("left", Assert.Single(vm.Placements).Edge);
    }

    [Fact]
    public async Task A_newer_daemon_protocol_is_reported_as_incompatible()
    {
        var client = new FakeDaemonClient { Hello = new HelloResponse(OmniProtocol.Version + 1, "9.9.9") };

        var vm = NewViewModel(client);
        await vm.RunAsync(CancellationToken.None); // returns immediately, no retry

        Assert.Equal(ConnectionStatus.Incompatible, vm.Connection);
        Assert.False(vm.IsConnected);
        Assert.Contains("Please update", vm.LastError);
        Assert.Empty(client.Calls);
    }

    [Fact]
    public async Task A_missing_daemon_goes_disconnected_and_keeps_retrying()
    {
        // Hello throws (daemon down) the first time, then we cancel so the loop ends.
        var client = new ThrowingThenStopClient();
        var vm = new DaemonViewModel(client, new ImmediateDispatcher(), reconnectDelay: TimeSpan.FromMinutes(5));

        await vm.RunAsync(client.Token);

        Assert.Equal(ConnectionStatus.Disconnected, vm.Connection);
        Assert.True(client.Attempts >= 1);
    }

    [Theory]
    [InlineData("accept", "ab12")]
    [InlineData("reject", "ab12")]
    [InlineData("connect", "10.0.0.2:4733")]
    [InlineData("remove_peer", "laptop")]
    public async Task Commands_forward_to_the_client(string command, string argument)
    {
        var client = new FakeDaemonClient();
        var vm = NewViewModel(client);

        Task call = command switch
        {
            "accept" => vm.AcceptAsync(argument),
            "reject" => vm.RejectAsync(argument),
            "connect" => vm.ConnectAsync(argument),
            "remove_peer" => vm.RemovePeerAsync(argument),
            _ => throw new ArgumentOutOfRangeException(nameof(command)),
        };
        await call;

        Assert.Contains((command, argument), client.Calls);
        Assert.Null(vm.LastError);
    }

    [Fact]
    public async Task Layout_and_clipboard_commands_forward_their_arguments()
    {
        var client = new FakeDaemonClient();
        var vm = NewViewModel(client);

        await vm.SetLayoutAsync("mac", "right");
        await vm.SetClipboardAsync(true);

        Assert.Contains(("layout", "mac:right"), client.Calls);
        Assert.Contains(("clipboard", "on"), client.Calls);
    }

    [Fact]
    public async Task A_failed_command_surfaces_its_message_in_last_error()
    {
        var client = new FakeDaemonClient { FailDisconnectWith = "no active session with mac" };
        var vm = NewViewModel(client);

        await vm.DisconnectAsync("mac");

        Assert.Equal("no active session with mac", vm.LastError);
    }

    [Fact]
    public async Task An_unexpected_error_does_not_kill_the_run_loop()
    {
        // A daemon that adds an Event variant stays protocol-compatible (see
        // ipc.rs), so the push stream can throw a decode error the client never
        // anticipated. That must degrade to "disconnected, retrying" — not stop
        // the loop and freeze the UI on stale data forever.
        var client = new UnexpectedFailureClient(failuresBeforeStopping: 2);
        var vm = new DaemonViewModel(client, new ImmediateDispatcher(), TimeSpan.FromMilliseconds(1));

        await vm.RunAsync(client.Token);

        Assert.Equal(ConnectionStatus.Disconnected, vm.Connection);
        Assert.Equal(2, client.Attempts);
    }

    [Fact]
    public async Task Reconnect_now_restarts_the_loop_without_waiting_for_the_delay()
    {
        // `Start daemon` calls this: the UI must go live at once rather than sit
        // on a multi-second reconnect sleep.
        var client = new CountingHelloClient();
        var vm = new DaemonViewModel(client, new ImmediateDispatcher(), TimeSpan.FromHours(1));

        vm.Start(CancellationToken.None);
        await client.WaitForHandshake();

        // The loop is now parked on a one-hour reconnect delay. Without a working
        // ReconnectNow this second handshake would never arrive.
        vm.ReconnectNow();
        await client.WaitForHandshake();

        await vm.ShutdownAsync();
        Assert.True(client.Attempts >= 2, $"expected at least 2 handshakes, saw {client.Attempts}");
    }

    [Fact]
    public async Task Start_is_only_offered_when_the_daemon_could_actually_start()
    {
        // Mirrors the macOS General pane: Start is disabled while connected and
        // while incompatible; Stop only while connected.
        var connected = await ConnectedViewModel();
        Assert.False(connected.CanStartDaemon);
        Assert.True(connected.CanStopDaemon);

        var incompatible = new FakeDaemonClient { Hello = new HelloResponse(OmniProtocol.Version + 1, "9.9.9") };
        var vm = NewViewModel(incompatible);
        await vm.RunAsync(CancellationToken.None);
        Assert.False(vm.CanStartDaemon);
        Assert.False(vm.CanStopDaemon);
        // An incompatible daemon replaces the pane instead of showing controls.
        Assert.False(vm.ShowDaemonSections);
    }

    [Fact]
    public async Task Connections_shows_the_empty_state_only_when_down_and_empty()
    {
        var vm = NewViewModel(new FakeDaemonClient());
        // Never connected, nothing known: the "Daemon Not Running" pane.
        Assert.True(vm.ShowConnectionsEmptyState);
        Assert.False(vm.ShowConnectionsContent);

        var connected = await ConnectedViewModel();
        Assert.False(connected.ShowConnectionsEmptyState);
        Assert.True(connected.ShowConnectionsContent);
    }

    [Fact]
    public async Task The_pending_badge_and_capture_text_follow_the_snapshot()
    {
        var vm = await ConnectedViewModel();

        Assert.Equal(1, vm.PendingCount);
        Assert.Equal("Active", vm.CaptureStatus);
        Assert.Equal("v0.3.7", vm.DisplayVersion);
    }

    [Fact]
    public void An_unknown_version_shows_nothing_rather_than_a_bare_v()
    {
        var vm = NewViewModel(new FakeDaemonClient());

        Assert.Equal("", vm.DisplayVersion);
    }

    /// <summary>A view model driven to the connected state by one snapshot.</summary>
    private static async Task<DaemonViewModel> ConnectedViewModel()
    {
        var client = new FakeDaemonClient();
        client.Snapshots.Add(Snapshot(capturing: true));
        using var cts = new CancellationTokenSource();
        client.OnSubscribeDrained = cts.Cancel;

        var vm = NewViewModel(client);
        await vm.RunAsync(cts.Token);
        return vm;
    }

    /// <summary>
    /// A client whose push stream throws something the view model does not know
    /// about, cancelling once it has been retried the requested number of times.
    /// </summary>
    private sealed class UnexpectedFailureClient : IOmniDaemonClient
    {
        private readonly CancellationTokenSource _cts = new();
        private readonly int _failuresBeforeStopping;

        public UnexpectedFailureClient(int failuresBeforeStopping)
        {
            _failuresBeforeStopping = failuresBeforeStopping;
        }

        public int Attempts { get; private set; }
        public CancellationToken Token => _cts.Token;

        public Task<HelloResponse> HelloAsync(CancellationToken cancellationToken = default)
        {
            Attempts++;
            if (Attempts >= _failuresBeforeStopping)
            {
                _cts.Cancel();
            }
            // Not an OmniDaemonException: this is the class of failure that used
            // to escape the loop entirely.
            throw new InvalidOperationException("unknown event 'peer_added'");
        }

        public Task<StatusInfo> StatusAsync(CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task<IReadOnlyList<PeerInfo>> PeersAsync(CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task<IReadOnlyList<LayoutInfo>> LayoutAsync(CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task ConnectAsync(string host, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task DisconnectAsync(string host, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task AcceptAsync(string selector, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task RejectAsync(string selector, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task RemovePeerAsync(string selector, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task SetLayoutAsync(string host, string edge, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task SetClipboardAsync(bool enabled, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task StopAsync(CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public IAsyncEnumerable<StatusInfo> SubscribeAsync(CancellationToken cancellationToken = default) => throw new NotSupportedException();
    }

    /// <summary>A client that counts handshakes so a restart can be observed.</summary>
    private sealed class CountingHelloClient : IOmniDaemonClient
    {
        /// <summary>
        /// One permit released per handshake, so each wait consumes exactly one.
        /// A single completion signal would not do: it stays completed, and the
        /// second wait would sail through without a second handshake happening.
        /// </summary>
        private readonly SemaphoreSlim _handshakes = new(0);

        private int _attempts;
        public int Attempts => Volatile.Read(ref _attempts);

        /// <summary>Waits for the next handshake, failing the test if none comes.</summary>
        public async Task WaitForHandshake()
        {
            Assert.True(
                await _handshakes.WaitAsync(TimeSpan.FromSeconds(5)),
                "expected another handshake attempt");
        }

        public Task<HelloResponse> HelloAsync(CancellationToken cancellationToken = default)
        {
            Interlocked.Increment(ref _attempts);
            _handshakes.Release();
            // Fail after the handshake so the loop parks in the reconnect delay,
            // which is exactly the state ReconnectNow has to break out of.
            throw new OmniDaemonException("the omni daemon is not running");
        }

        public Task<StatusInfo> StatusAsync(CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task<IReadOnlyList<PeerInfo>> PeersAsync(CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task<IReadOnlyList<LayoutInfo>> LayoutAsync(CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task ConnectAsync(string host, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task DisconnectAsync(string host, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task AcceptAsync(string selector, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task RejectAsync(string selector, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task RemovePeerAsync(string selector, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task SetLayoutAsync(string host, string edge, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task SetClipboardAsync(bool enabled, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task StopAsync(CancellationToken cancellationToken = default) => Task.CompletedTask;
        public IAsyncEnumerable<StatusInfo> SubscribeAsync(CancellationToken cancellationToken = default) => throw new NotSupportedException();
    }

    /// <summary>A client whose handshake always fails, cancelling after one attempt.</summary>
    private sealed class ThrowingThenStopClient : IOmniDaemonClient
    {
        private readonly CancellationTokenSource _cts = new();
        public int Attempts { get; private set; }
        public CancellationToken Token => _cts.Token;

        public Task<HelloResponse> HelloAsync(CancellationToken cancellationToken = default)
        {
            Attempts++;
            _cts.Cancel(); // end the retry loop after this attempt
            throw new OmniDaemonException("the omni daemon is not running");
        }

        public Task<StatusInfo> StatusAsync(CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task<IReadOnlyList<PeerInfo>> PeersAsync(CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task<IReadOnlyList<LayoutInfo>> LayoutAsync(CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task ConnectAsync(string host, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task DisconnectAsync(string host, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task AcceptAsync(string selector, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task RejectAsync(string selector, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task RemovePeerAsync(string selector, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task SetLayoutAsync(string host, string edge, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task SetClipboardAsync(bool enabled, CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public Task StopAsync(CancellationToken cancellationToken = default) => throw new NotSupportedException();
        public IAsyncEnumerable<StatusInfo> SubscribeAsync(CancellationToken cancellationToken = default) => throw new NotSupportedException();
    }
}

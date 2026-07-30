using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Omni.App.Components;
using Omni.App.Core;
using Windows.System;

namespace Omni.App.Views;

/// <summary>
/// The Connections pane: dial a host, answer incoming requests, manage live
/// sessions and known peers, and place each peer around the screen. Mirrors the
/// macOS Connections pane, including which selector each action sends.
/// </summary>
public sealed partial class ConnectionsView : UserControl
{
    public DaemonViewModel ViewModel { get; }

    public ConnectionsView(DaemonViewModel viewModel)
    {
        ViewModel = viewModel;
        InitializeComponent();
    }

    // Connect is offered only once there is a host to dial, as on macOS.
    private void OnHostInputChanged(object sender, TextChangedEventArgs e) =>
        ConnectButton.IsEnabled = HostInput.Text.Trim().Length > 0;

    private async void OnHostInputKeyDown(object sender, Microsoft.UI.Xaml.Input.KeyRoutedEventArgs e)
    {
        if (e.Key == VirtualKey.Enter)
        {
            e.Handled = true;
            await SubmitConnectAsync();
        }
    }

    private async void OnConnectClick(object sender, RoutedEventArgs e) => await SubmitConnectAsync();

    private async Task SubmitConnectAsync()
    {
        var host = HostInput.Text.Trim();
        if (host.Length == 0)
        {
            return;
        }
        await ViewModel.ConnectAsync(host);
        HostInput.Text = "";
    }

    private async void OnAcceptClick(object sender, RoutedEventArgs e)
    {
        // The fingerprint is the selector: it is what the user just verified.
        if (sender is PendingRequestCard { Request: { } request })
        {
            await ViewModel.AcceptAsync(request.Fingerprint);
        }
    }

    private async void OnRejectClick(object sender, RoutedEventArgs e)
    {
        if (sender is PendingRequestCard { Request: { } request })
        {
            await ViewModel.RejectAsync(request.Fingerprint);
        }
    }

    private async void OnDisconnectClick(object sender, RoutedEventArgs e)
    {
        if (sender is SessionCard { Session: { } session })
        {
            await ViewModel.DisconnectAsync(session.Host);
        }
    }

    private async void OnForgetPeerClick(object sender, RoutedEventArgs e)
    {
        // Prefer the host, falling back to the fingerprint for an unnamed peer —
        // the same selector the macOS pane sends.
        if (sender is PeerCard { Peer: { } peer })
        {
            await ViewModel.RemovePeerAsync(peer.Host ?? peer.Fingerprint);
        }
    }

    private async void OnLayoutEdgeChanged(object sender, RoutedEventArgs e)
    {
        if (sender is LayoutRow { Placement: { } placement, SelectedEdge: { } edge })
        {
            await ViewModel.SetLayoutAsync(placement.Host, edge);
        }
    }
}

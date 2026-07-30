using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Omni.Ipc;

namespace Omni.App.Components;

public sealed partial class PeerCard : UserControl
{
    public PeerInfo? Peer
    {
        get => (PeerInfo?)GetValue(PeerProperty);
        set => SetValue(PeerProperty, value);
    }

    public static readonly DependencyProperty PeerProperty =
        DependencyProperty.Register(
            nameof(Peer),
            typeof(PeerInfo),
            typeof(PeerCard),
            new PropertyMetadata(null));

    public string ButtonLabel { get; set; } = "Forget";

    public event RoutedEventHandler? ActionClicked;

    public PeerCard()
    {
        InitializeComponent();
    }

    /// <summary>The peer's host, or a placeholder when it was never named.</summary>
    public string HostLabel(PeerInfo? peer) => peer?.Host ?? "(unnamed)";

    /// <summary>The pinned certificate fingerprint.</summary>
    public string FingerprintLabel(PeerInfo? peer) => peer?.Fingerprint ?? "";

    private void OnButtonClick(object sender, RoutedEventArgs e) => ActionClicked?.Invoke(this, e);
}

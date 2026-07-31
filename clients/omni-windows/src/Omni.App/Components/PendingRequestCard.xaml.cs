using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Omni.Ipc;

namespace Omni.App.Components;

public sealed partial class PendingRequestCard : UserControl
{
    public PendingInfo? Request
    {
        get => (PendingInfo?)GetValue(RequestProperty);
        set => SetValue(RequestProperty, value);
    }

    public static readonly DependencyProperty RequestProperty =
        DependencyProperty.Register(
            nameof(Request),
            typeof(PendingInfo),
            typeof(PendingRequestCard),
            new PropertyMetadata(null));

    public event RoutedEventHandler? AcceptClicked;
    public event RoutedEventHandler? RejectClicked;

    public PendingRequestCard()
    {
        InitializeComponent();
    }

    /// <summary>The host asking for control.</summary>
    public string HostLabel(PendingInfo? request) => request?.Host ?? "";

    /// <summary>The fingerprint the user verifies before accepting.</summary>
    public string FingerprintLabel(PendingInfo? request) => request?.Fingerprint ?? "";

    // The card itself is the sender, so a handler can read its data directly.
    private void OnAcceptClick(object sender, RoutedEventArgs e) => AcceptClicked?.Invoke(this, e);

    private void OnRejectClick(object sender, RoutedEventArgs e) => RejectClicked?.Invoke(this, e);
}

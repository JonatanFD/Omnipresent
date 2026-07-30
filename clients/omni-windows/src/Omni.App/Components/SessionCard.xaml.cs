using System.Globalization;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Omni.Ipc;

namespace Omni.App.Components;

public sealed partial class SessionCard : UserControl
{
    public SessionInfo? Session
    {
        get => (SessionInfo?)GetValue(SessionProperty);
        set => SetValue(SessionProperty, value);
    }

    public static readonly DependencyProperty SessionProperty =
        DependencyProperty.Register(
            nameof(Session),
            typeof(SessionInfo),
            typeof(SessionCard),
            new PropertyMetadata(null));

    public string ButtonLabel { get; set; } = "Disconnect";

    public event RoutedEventHandler? ActionClicked;

    public SessionCard()
    {
        InitializeComponent();
    }

    /// <summary>The peer on the other end of this session.</summary>
    public string HostLabel(SessionInfo? session) => session?.Host ?? "";

    /// <summary>
    /// This machine's role, capitalized for display. The daemon sends it
    /// lowercase ("controller" / "target").
    /// </summary>
    public string RoleLabel(SessionInfo? session) => Capitalize(session?.Role ?? "");

    /// <summary>Whether to show the indicator that input is routed here.</summary>
    public Visibility ActiveIndicator(SessionInfo? session) =>
        session?.Active == true ? Visibility.Visible : Visibility.Collapsed;

    private static string Capitalize(string text) =>
        text.Length == 0
            ? text
            : string.Concat(text[..1].ToUpper(CultureInfo.CurrentCulture), text[1..]);

    private void OnButtonClick(object sender, RoutedEventArgs e) => ActionClicked?.Invoke(this, e);
}

using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Omni.App.Core;
using Omni.App.Views;

namespace Omni.App;

/// <summary>
/// The application shell: a navigation pane beside the section that is showing.
/// Sections match the macOS client one for one — General, Connections, System,
/// Update — and the section name becomes the detail header, the way the macOS
/// panes set their navigation title.
/// </summary>
public sealed partial class MainView : UserControl
{
    /// <summary>The sections, in pane order, with the title each one shows.</summary>
    private static readonly (string Tag, string Title)[] Sections =
    [
        ("general", "General"),
        ("connections", "Connections"),
        ("system", "System"),
        ("update", "Update"),
    ];

    /// <summary>
    /// Panes are built once and reused, so navigating away and back does not
    /// discard what the user typed.
    /// </summary>
    private readonly Dictionary<string, UserControl> _panes = [];

    public DaemonViewModel ViewModel { get; }

    public MainView(DaemonViewModel viewModel)
    {
        ViewModel = viewModel;
        InitializeComponent();
        Loaded += OnLoaded;
    }

    private void OnLoaded(object sender, RoutedEventArgs e)
    {
        // General is the landing section, as on macOS.
        NavView.SelectedItem = NavView.MenuItems[0];
        NavigateToSection(Sections[0].Tag);
    }

    private void OnNavItemInvoked(NavigationView sender, NavigationViewItemInvokedEventArgs args)
    {
        var tag = (args.InvokedItemContainer as NavigationViewItem)?.Tag as string;
        NavigateToSection(tag ?? Sections[0].Tag);
    }

    private void NavigateToSection(string tag)
    {
        // An unknown tag lands on the first section rather than a blank pane.
        var index = Array.FindIndex(Sections, s => s.Tag == tag);
        var (canonicalTag, title) = Sections[index < 0 ? 0 : index];

        if (!_panes.TryGetValue(canonicalTag, out var pane))
        {
            pane = CreatePane(canonicalTag);
            _panes[canonicalTag] = pane;
        }
        NavView.Header = title;
        ContentFrame.Content = pane;
    }

    private UserControl CreatePane(string tag) => tag switch
    {
        "connections" => new ConnectionsView(ViewModel),
        "system" => new SystemView(ViewModel),
        "update" => new UpdateView(ViewModel),
        _ => new GeneralView(ViewModel),
    };
}

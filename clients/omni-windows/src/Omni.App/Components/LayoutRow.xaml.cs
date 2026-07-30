using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Omni.Ipc;

namespace Omni.App.Components;

/// <summary>
/// One peer's placement in the virtual desktop: the host and a picker for the
/// edge it sits past. Selecting an edge raises <see cref="EdgeChanged"/> straight
/// away, mirroring the macOS pane where the Picker applies immediately.
/// </summary>
public sealed partial class LayoutRow : UserControl
{
    /// <summary>The four edges, in the order the picker lists them.</summary>
    private static readonly string[] Edges = ["left", "right", "top", "bottom"];

    public LayoutInfo? Placement
    {
        get => (LayoutInfo?)GetValue(PlacementProperty);
        set => SetValue(PlacementProperty, value);
    }

    public static readonly DependencyProperty PlacementProperty =
        DependencyProperty.Register(nameof(Placement), typeof(LayoutInfo), typeof(LayoutRow),
            new PropertyMetadata(null));

    /// <summary>The edge the user picked, lowercase as the daemon expects it.</summary>
    public string? SelectedEdge { get; private set; }

    /// <summary>Raised when the user picks a different edge for this host.</summary>
    public event RoutedEventHandler? EdgeChanged;

    public LayoutRow()
    {
        InitializeComponent();
    }

    /// <summary>The host this row places.</summary>
    public string HostLabel(LayoutInfo? placement) => placement?.Host ?? "";

    /// <summary>Which picker entry matches the placement the daemon reports.</summary>
    public int SelectedEdgeIndex(LayoutInfo? placement)
    {
        var index = Array.IndexOf(Edges, placement?.Edge?.ToLowerInvariant());
        return index < 0 ? 0 : index;
    }

    private void OnEdgeSelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        var index = EdgePicker.SelectedIndex;
        if (index < 0 || index >= Edges.Length || Placement is null)
        {
            return;
        }
        var edge = Edges[index];
        // The picker also fires while it is being populated with the daemon's
        // current value; only a genuine change is worth a round trip.
        if (string.Equals(edge, Placement.Edge, StringComparison.OrdinalIgnoreCase))
        {
            return;
        }
        SelectedEdge = edge;
        EdgeChanged?.Invoke(this, new RoutedEventArgs());
    }
}

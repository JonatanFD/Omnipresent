using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;

namespace Omni.App.Components;

/// <summary>
/// One "label — value" row, the counterpart of SwiftUI's <c>LabeledContent</c>
/// used throughout the macOS panes. Keeps every information row in the app
/// arranged the same way.
/// </summary>
public sealed partial class LabeledRow : UserControl
{
    public string Label
    {
        get => (string)GetValue(LabelProperty);
        set => SetValue(LabelProperty, value);
    }

    public static readonly DependencyProperty LabelProperty =
        DependencyProperty.Register(nameof(Label), typeof(string), typeof(LabeledRow),
            new PropertyMetadata(""));

    public string Value
    {
        get => (string)GetValue(ValueProperty);
        set => SetValue(ValueProperty, value);
    }

    public static readonly DependencyProperty ValueProperty =
        DependencyProperty.Register(nameof(Value), typeof(string), typeof(LabeledRow),
            new PropertyMetadata(""));

    /// <summary>Renders the value in a monospaced face, for fingerprints.</summary>
    public bool Monospaced
    {
        get => (bool)GetValue(MonospacedProperty);
        set => SetValue(MonospacedProperty, value);
    }

    public static readonly DependencyProperty MonospacedProperty =
        DependencyProperty.Register(nameof(Monospaced), typeof(bool), typeof(LabeledRow),
            new PropertyMetadata(false));

    /// <summary>Lets the user select the value, for copyable identifiers.</summary>
    public bool Selectable
    {
        get => (bool)GetValue(SelectableProperty);
        set => SetValue(SelectableProperty, value);
    }

    public static readonly DependencyProperty SelectableProperty =
        DependencyProperty.Register(nameof(Selectable), typeof(bool), typeof(LabeledRow),
            new PropertyMetadata(false));

    public LabeledRow()
    {
        InitializeComponent();
    }

    /// <summary>The face for the value, monospaced only when asked.</summary>
    public FontFamily ValueFontFamily(bool monospaced) =>
        monospaced ? new FontFamily("Consolas") : FontFamily;
}

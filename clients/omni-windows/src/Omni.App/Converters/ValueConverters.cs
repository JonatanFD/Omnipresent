using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Data;
using Microsoft.UI.Xaml.Media;
using Omni.App.Core;

namespace Omni.App.Converters;

public sealed class BoolToVisibilityConverter : IValueConverter
{
    public object Convert(object value, Type targetType, object parameter, string language)
    {
        var boolValue = (bool?)value ?? false;
        var invert = parameter as string == "Invert";
        return (boolValue ^ invert) ? Visibility.Visible : Visibility.Collapsed;
    }

    public object ConvertBack(object value, Type targetType, object parameter, string language)
    {
        return (Visibility)value == Visibility.Visible;
    }
}

/// <summary>
/// Paints the daemon's state: green when live, amber while trying or when the
/// app is too old, and muted when it is simply not there. The same colour
/// meanings the macOS sidebar dot uses.
/// </summary>
public sealed class ConnectionStatusBrushConverter : IValueConverter
{
    public object Convert(object value, Type targetType, object parameter, string language)
    {
        var key = (value as ConnectionStatus?) switch
        {
            ConnectionStatus.Connected => "SystemFillColorSuccessBrush",
            ConnectionStatus.Connecting => "SystemFillColorCautionBrush",
            ConnectionStatus.Incompatible => "SystemFillColorCautionBrush",
            _ => "TextFillColorSecondaryBrush",
        };
        return Application.Current.Resources.TryGetValue(key, out var brush)
            ? brush
            : new SolidColorBrush(Microsoft.UI.Colors.Gray);
    }

    public object ConvertBack(object value, Type targetType, object parameter, string language) =>
        throw new NotSupportedException();
}

/// <summary>
/// The glyph beside the status text. These are Segoe Fluent Icons code points,
/// picked to read the way the macOS General pane's symbols do.
/// </summary>
public sealed class ConnectionStatusGlyphConverter : IValueConverter
{
    // Code points rather than the characters themselves: these live in Unicode's
    // private use area, where a literal is invisible in most editors and diffs.
    private const int Completed = 0xE930;  // filled circle with a check
    private const int Pending = 0xE712;    // horizontal ellipsis
    private const int Warning = 0xE7BA;    // filled triangle
    private const int Cancelled = 0xE711;  // cross

    public object Convert(object value, Type targetType, object parameter, string language)
    {
        var glyph = (value as ConnectionStatus?) switch
        {
            ConnectionStatus.Connected => Completed,
            ConnectionStatus.Connecting => Pending,
            ConnectionStatus.Incompatible => Warning,
            _ => Cancelled,
        };
        return char.ConvertFromUtf32(glyph);
    }

    public object ConvertBack(object value, Type targetType, object parameter, string language) =>
        throw new NotSupportedException();
}

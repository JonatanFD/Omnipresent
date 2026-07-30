using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Omni.App.Core;

namespace Omni.App.Views;

/// <summary>
/// The General pane: the daemon's state, the Start/Stop controls, and what the
/// daemon reports about itself. Mirrors the macOS General pane; every visibility
/// and enablement rule comes from the view model, so this is only binding.
/// </summary>
public sealed partial class GeneralView : UserControl
{
    public DaemonViewModel ViewModel { get; }

    public GeneralView(DaemonViewModel viewModel)
    {
        ViewModel = viewModel;
        InitializeComponent();
    }

    private async void OnStartClick(object sender, RoutedEventArgs e)
    {
        await ViewModel.StartDaemonAsync();
    }

    private async void OnStopClick(object sender, RoutedEventArgs e)
    {
        await ViewModel.StopDaemonAsync();
    }
}

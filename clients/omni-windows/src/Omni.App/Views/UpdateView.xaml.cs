using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Omni.App.Core;

namespace Omni.App.Views;

/// <summary>
/// The Update pane: the installed version, and a button that runs
/// <c>omni update</c>. Mirrors the macOS Update pane, including swapping the
/// button's label for a spinner while the update runs.
/// </summary>
public sealed partial class UpdateView : UserControl
{
    public DaemonViewModel ViewModel { get; }

    public UpdateView(DaemonViewModel viewModel)
    {
        ViewModel = viewModel;
        InitializeComponent();
    }

    private async void OnUpdateClick(object sender, RoutedEventArgs e)
    {
        SetUpdating(true);
        UpdateMessage.Visibility = Visibility.Collapsed;

        // Finding and running the CLI lives in the view model, so this pane and
        // the Start button agree on where the binary is.
        var message = await ViewModel.RunUpdateAsync();

        UpdateMessage.Text = message;
        UpdateMessage.Visibility = Visibility.Visible;
        SetUpdating(false);
    }

    /// <summary>Shows progress in place of the label while the update runs.</summary>
    private void SetUpdating(bool updating)
    {
        UpdateButton.IsEnabled = !updating;
        UpdateProgress.IsActive = updating;
        UpdateProgress.Visibility = updating ? Visibility.Visible : Visibility.Collapsed;
        UpdateButtonLabel.Text = updating ? "Updating…" : "Update Omnipresent";
    }
}

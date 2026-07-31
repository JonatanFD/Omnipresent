using Omni.App.Core;

namespace Omni.App.Tests;

/// <summary>
/// The locator has to agree with <c>install.ps1</c>, otherwise the GUI cannot
/// start or update the daemon for anyone who installed the documented way.
/// </summary>
public class OmniBinaryLocatorTests
{
    private const string LocalAppData = @"C:\Users\x\AppData\Local";
    private const string ProgramFiles = @"C:\Program Files";
    private const string UserProfile = @"C:\Users\x";

    /// <summary>A locator over a make-believe disk and environment.</summary>
    private static OmniBinaryLocator Locator(
        string[] onDisk,
        Dictionary<string, string>? environment = null)
    {
        var present = new HashSet<string>(onDisk, StringComparer.OrdinalIgnoreCase);
        var env = environment ?? new Dictionary<string, string>
        {
            ["LOCALAPPDATA"] = LocalAppData,
            ["ProgramFiles"] = ProgramFiles,
            ["USERPROFILE"] = UserProfile,
        };
        return new OmniBinaryLocator(
            fileExists: present.Contains,
            environment: name => env.TryGetValue(name, out var value) ? value : null);
    }

    [Fact]
    public void Finds_the_binary_where_the_installer_puts_it()
    {
        // install.ps1 installs to %LOCALAPPDATA%\Programs\omni by default.
        var expected = Path.Combine(LocalAppData, "Programs", "omni", "omni.exe");

        Assert.Equal(expected, Locator([expected]).Locate());
    }

    [Fact]
    public void The_installer_override_wins_over_every_default()
    {
        var overridden = @"D:\tools\omni\omni.exe";
        var installed = Path.Combine(LocalAppData, "Programs", "omni", "omni.exe");
        var locator = Locator(
            onDisk: [overridden, installed],
            environment: new Dictionary<string, string>
            {
                ["OMNI_INSTALL_DIR"] = @"D:\tools\omni",
                ["LOCALAPPDATA"] = LocalAppData,
            });

        Assert.Equal(overridden, locator.Locate());
    }

    [Fact]
    public void A_machine_wide_install_is_found()
    {
        var expected = Path.Combine(ProgramFiles, "Omnipresent", "omni.exe");

        Assert.Equal(expected, Locator([expected]).Locate());
    }

    [Fact]
    public void A_cargo_install_is_found()
    {
        var expected = Path.Combine(UserProfile, ".cargo", "bin", "omni.exe");

        Assert.Equal(expected, Locator([expected]).Locate());
    }

    [Fact]
    public void Anything_on_path_is_found_as_a_last_resort()
    {
        var expected = @"E:\bin\omni.exe";
        var locator = Locator(
            onDisk: [expected],
            environment: new Dictionary<string, string> { ["PATH"] = @"C:\windows;E:\bin" });

        Assert.Equal(expected, locator.Locate());
    }

    [Fact]
    public void An_empty_or_broken_path_entry_is_skipped_rather_than_throwing()
    {
        var expected = @"E:\bin\omni.exe";
        var locator = Locator(
            onDisk: [expected],
            environment: new Dictionary<string, string> { ["PATH"] = @";  ;E:\bin;" });

        Assert.Equal(expected, locator.Locate());
    }

    [Fact]
    public void Locate_returns_null_when_the_binary_is_nowhere()
    {
        Assert.Null(Locator([]).Locate());
    }

    [Fact]
    public void Candidates_are_offered_most_specific_first()
    {
        var locator = Locator(
            onDisk: [],
            environment: new Dictionary<string, string>
            {
                ["OMNI_INSTALL_DIR"] = @"D:\tools\omni",
                ["LOCALAPPDATA"] = LocalAppData,
                ["ProgramFiles"] = ProgramFiles,
                ["USERPROFILE"] = UserProfile,
                ["PATH"] = @"E:\bin",
            });

        var expected = new[]
        {
            @"D:\tools\omni\omni.exe",
            Path.Combine(LocalAppData, "Programs", "omni", "omni.exe"),
            Path.Combine(ProgramFiles, "Omnipresent", "omni.exe"),
            Path.Combine(UserProfile, ".cargo", "bin", "omni.exe"),
            @"E:\bin\omni.exe",
        };

        Assert.Equal(expected, locator.Candidates());
    }

    [Fact]
    public void A_missing_environment_yields_no_candidates_instead_of_throwing()
    {
        var locator = new OmniBinaryLocator(fileExists: _ => false, environment: _ => null);

        Assert.Empty(locator.Candidates());
        Assert.Null(locator.Locate());
    }
}

using System.Reflection;
using System.Text.RegularExpressions;

namespace Omni.App.Tests;

/// <summary>
/// Guards the XAML values that are resolved when the app *runs*, not when it
/// builds.
///
/// <c>Icon="Network"</c> compiles perfectly happily — there is no such member of
/// WinUI's <c>Symbol</c> enum, but nothing checks that until the parser reaches
/// it at startup, where it raises <c>E_XAMLPARSEFAILED</c> and takes the window
/// down before it ever appears. The GUI shipped that way through several
/// releases and could not be opened at all, because every gate we had was a
/// build, and the build was clean.
///
/// This reads the shipped XAML and checks the icon names against the real enum,
/// so the same mistake fails here instead of on someone's desktop.
/// </summary>
public class XamlResourceTests
{
    /// <summary>
    /// The icons the app actually uses, each confirmed against the real enum.
    /// Used only when the WinUI assembly cannot be found, so a machine without it
    /// still fails on an unknown name rather than waving everything through.
    /// </summary>
    private static readonly HashSet<string> KnownGoodSymbols =
        ["Home", "Globe", "Setting", "Download"];

    [Fact]
    public void Every_icon_name_in_the_xaml_is_a_real_symbol()
    {
        // Prefer the enum itself; fall back to the confirmed set rather than
        // passing vacuously when the assembly is not around.
        var symbols = LoadSymbolNames();
        if (symbols.Count == 0)
        {
            symbols = KnownGoodSymbols;
        }

        var offenders = new List<string>();
        foreach (var file in XamlFiles())
        {
            foreach (Match match in Regex.Matches(File.ReadAllText(file), @"Icon\s*=\s*""([A-Za-z]+)"""))
            {
                var name = match.Groups[1].Value;
                if (!symbols.Contains(name))
                {
                    offenders.Add($"{Path.GetFileName(file)}: Icon=\"{name}\"");
                }
            }
        }

        Assert.True(
            offenders.Count == 0,
            "these icon names are not members of WinUI's Symbol enum, so the XAML parser "
                + "fails at startup and the window never opens:\n  "
                + string.Join("\n  ", offenders));
    }

    [Fact]
    public void The_xaml_files_are_actually_being_found()
    {
        // If the search ever stops finding them the icon test would pass
        // vacuously, which is worse than failing.
        Assert.NotEmpty(XamlFiles());
    }

    /// <summary>The app's XAML files, found by walking up to the client root.</summary>
    private static List<string> XamlFiles()
    {
        var dir = new DirectoryInfo(AppContext.BaseDirectory);
        while (dir is not null && !Directory.Exists(Path.Combine(dir.FullName, "src", "Omni.App")))
        {
            dir = dir.Parent;
        }
        if (dir is null)
        {
            return [];
        }
        return [.. Directory.EnumerateFiles(
            Path.Combine(dir.FullName, "src", "Omni.App"), "*.xaml", SearchOption.AllDirectories)];
    }

    /// <summary>
    /// The <c>Symbol</c> member names, read from whichever copy of the WinUI
    /// assembly this machine has — beside the tests, in the app's build output,
    /// or in the NuGet package cache.
    /// </summary>
    private static HashSet<string> LoadSymbolNames()
    {
        foreach (var dll in CandidateAssemblies())
        {
            try
            {
                var symbol = Assembly.LoadFrom(dll).GetType("Microsoft.UI.Xaml.Controls.Symbol");
                if (symbol is { IsEnum: true })
                {
                    return [.. Enum.GetNames(symbol)];
                }
            }
            catch
            {
                // Try the next copy.
            }
        }
        return [];
    }

    private static IEnumerable<string> CandidateAssemblies()
    {
        const string name = "Microsoft.WinUI.dll";
        var roots = new List<string> { AppContext.BaseDirectory };

        var dir = new DirectoryInfo(AppContext.BaseDirectory);
        while (dir is not null && !Directory.Exists(Path.Combine(dir.FullName, "src", "Omni.App")))
        {
            dir = dir.Parent;
        }
        if (dir is not null)
        {
            roots.Add(Path.Combine(dir.FullName, "src", "Omni.App", "bin"));
        }

        var nuget = Environment.GetEnvironmentVariable("NUGET_PACKAGES")
            ?? Path.Combine(
                Environment.GetFolderPath(Environment.SpecialFolder.UserProfile),
                ".nuget", "packages");
        roots.Add(Path.Combine(nuget, "microsoft.windowsappsdk"));

        foreach (var root in roots)
        {
            if (!Directory.Exists(root))
            {
                continue;
            }
            IEnumerable<string> found;
            try
            {
                found = Directory.EnumerateFiles(root, name, SearchOption.AllDirectories);
            }
            catch
            {
                continue;
            }
            foreach (var file in found)
            {
                yield return file;
            }
        }
    }
}

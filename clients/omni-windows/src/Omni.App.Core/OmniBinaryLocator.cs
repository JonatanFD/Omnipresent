namespace Omni.App.Core;

/// <summary>
/// Finds the <c>omni.exe</c> CLI so the app can start, stop, or update the daemon.
///
/// The order mirrors how the binary actually gets onto a machine, most specific
/// first: the installer's own <c>OMNI_INSTALL_DIR</c> override, then
/// <c>install.ps1</c>'s per-user default, then a machine-wide install, then a
/// <c>cargo install</c> build, and finally anything on <c>PATH</c>.
///
/// The filesystem and environment are injected so the order can be unit-tested
/// without a real install.
/// </summary>
public sealed class OmniBinaryLocator
{
    /// <summary>The CLI's file name on Windows.</summary>
    public const string ExecutableName = "omni.exe";

    /// <summary>Windows separates <c>PATH</c> entries with a semicolon.</summary>
    private const char PathSeparator = ';';

    private readonly Func<string, bool> _fileExists;
    private readonly Func<string, string?> _environment;

    /// <param name="fileExists">Whether a path is a file. Defaults to the real filesystem.</param>
    /// <param name="environment">Reads an environment variable. Defaults to the real environment.</param>
    public OmniBinaryLocator(
        Func<string, bool>? fileExists = null,
        Func<string, string?>? environment = null)
    {
        _fileExists = fileExists ?? File.Exists;
        _environment = environment ?? Environment.GetEnvironmentVariable;
    }

    /// <summary>The locator over the real machine.</summary>
    public static OmniBinaryLocator Default { get; } = new();

    /// <summary>
    /// Every place the binary might be, in the order they are tried. Exposed so a
    /// failure message can tell the user where the app actually looked.
    /// </summary>
    public IReadOnlyList<string> Candidates()
    {
        var candidates = new List<string>();

        // The installer's override: a user who set it means it.
        AddDirectory(candidates, _environment("OMNI_INSTALL_DIR"));
        // install.ps1's per-user default, which needs no admin rights.
        AddDirectory(candidates, Combine(_environment("LOCALAPPDATA"), "Programs", "omni"));
        // A machine-wide install.
        AddDirectory(candidates, Combine(_environment("ProgramFiles"), "Omnipresent"));
        // Built from source with `cargo install`.
        AddDirectory(candidates, Combine(_environment("USERPROFILE"), ".cargo", "bin"));
        // Last resort: anything on PATH, which is the least specific answer.
        foreach (var directory in SplitPath(_environment("PATH")))
        {
            AddDirectory(candidates, directory);
        }

        return candidates;
    }

    /// <summary>
    /// The first candidate that exists, or <c>null</c> when the CLI is not
    /// installed anywhere the app knows to look.
    /// </summary>
    public string? Locate() => Candidates().FirstOrDefault(_fileExists);

    /// <summary>
    /// What to tell the user when the CLI is nowhere to be found, including the
    /// way out (the installer's own override).
    /// </summary>
    public static string NotFoundMessage() =>
        "Could not find omni.exe. Make sure Omnipresent is installed, or set " +
        "OMNI_INSTALL_DIR to the folder that holds it.";

    /// <summary>Appends <c>directory\omni.exe</c>, skipping blank directories.</summary>
    private static void AddDirectory(List<string> candidates, string? directory)
    {
        if (string.IsNullOrWhiteSpace(directory))
        {
            return;
        }
        candidates.Add(Path.Combine(directory.Trim(), ExecutableName));
    }

    /// <summary>Joins path parts, or returns null when the root is unset.</summary>
    private static string? Combine(string? root, params string[] parts) =>
        string.IsNullOrWhiteSpace(root) ? null : Path.Combine([root.Trim(), .. parts]);

    /// <summary>Splits a <c>PATH</c> value, dropping blank entries.</summary>
    private static string[] SplitPath(string? path) =>
        string.IsNullOrEmpty(path)
            ? []
            : path.Split(
                PathSeparator,
                StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries);
}

//! `hades --completions <SHELL>`: prints a shell completion script.

use std::io::Write;
use std::path::Path;

use clap::CommandFactory;
use clap_complete::Shell;

use crate::cli::CliArgs;

/// Name the completion script registers for.
///
/// The binary is installed as `hades` by cargo and as `hadey` by the npm package, so
/// the name the user actually typed (`argv[0]`) is used, falling back to `hades`.
pub fn invoked_bin_name() -> String {
    std::env::args_os()
        .next()
        .and_then(|arg0| {
            Path::new(&arg0)
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "hades".to_string())
}

/// Writes the completion script for `shell` to `out`.
pub fn write_completions<W: Write>(shell: Shell, bin_name: &str, out: &mut W) {
    let mut command = CliArgs::command();
    clap_complete::generate(shell, &mut command, bin_name, out);
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn script(shell: Shell) -> String {
        let mut out = Vec::new();
        write_completions(shell, "hadey", &mut out);
        String::from_utf8(out).expect("utf-8 script")
    }

    #[test]
    fn test_generates_scripts_for_every_supported_shell() {
        for shell in [
            Shell::Bash,
            Shell::Zsh,
            Shell::Fish,
            Shell::PowerShell,
            Shell::Elvish,
        ] {
            let text = script(shell);
            assert!(text.contains("hadey"), "{shell} script names the binary");
            assert!(text.contains("prompt"), "{shell} script lists --prompt");
        }
    }

    #[test]
    fn test_scripts_include_subcommands_and_flags() {
        let bash = script(Shell::Bash);
        assert!(bash.contains("mcp-server"));
        assert!(bash.contains("--session"));
        assert!(bash.contains("--completions"));
        assert!(script(Shell::PowerShell).contains("Register-ArgumentCompleter"));
    }

    #[test]
    fn test_completions_flag_parses_shell_names() {
        let args = CliArgs::try_parse_from(["hades", "--completions", "powershell"]).unwrap();
        assert_eq!(args.completions, Some(Shell::PowerShell));
        assert!(CliArgs::try_parse_from(["hades", "--completions", "tcsh"]).is_err());
    }
}

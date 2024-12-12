use std::fmt::Display;
use std::borrow::Cow;
use std::path::Path;

use indoc::formatdoc;

use crate::shell::Shell;

#[derive(Default)]
pub struct Pwsh {}

fn powershell_escape(s: Cow<str>) -> Cow<str> {
    let mut needs_escape = s.is_empty();

    for ch in s.chars() {
        match ch {
            '"' | '`' | '\'' | ' ' => needs_escape = true,
            _ => {}
        }
    }
    if !needs_escape {
        return s;
    }

    let mut es = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    loop {
        match chars.next() {
            Some('\t') => {
                es.push_str("`t");
            }
            Some('\n') => {
                es.push_str("`n");
            }
            Some('\r') => {
                es.push_str("`r");
            }
            Some(c) => {
                es.push(c);
            }
            None => {
                break;
            }
        }

    }
    es.push('"');
    es.into()
}

impl Shell for Pwsh {
    fn activate(&self, exe: &Path, flags: String) -> String {
        let exe = exe.to_string_lossy();
        let mut out = String::new();

        out.push_str(&formatdoc! {r#"
            $env:MISE_SHELL = "pwsh"
            $env:__MISE_ORIG_PATH = $env:PATH

            function mise {{

                $code = [System.Management.Automation.Language.Parser]::ParseInput($MyInvocation.Line.Substring($MyInvocation.OffsetInLine - 1), [ref]$null, [ref]$null)
                $myLine = $code.Find({{ $args[0].CommandElements }}, $true).CommandElements | ForEach-Object {{ $_.ToString() }} | Join-String -Separator ' '
                $command, [array]$arguments = Invoke-Expression ('Write-Output -- ' + $myLine)
                if ($null -eq $arguments) {{ $arguments = @() }} 


                if ($arguments.Lenght -eq 0) {{
                    & {exe}
                    return
                }}

                $command = $arguments[0]
                $arguments = $arguments[1..$arguments.Length]

                if ($arguments -contains "--help") {{
                    return & {exe} $command $arguments 
                }}

                switch ($command) {{
                    {{ $_ -in "deactivate", "shell", "sh" }} {{
                        if ($arguments -contains "-h" -or $arguments -contains "--help") {{
                            & {exe} $command $arguments
                        }}
                        else {{
                            & {exe} $command $arguments | Out-String | Invoke-Expression -ErrorAction SilentlyContinue
                        }}
                    }}
                    default {{
                        & {exe} $command $arguments
                    }}
                }}
            }}

            function _mise_hook {{
                & {exe} hook-env{flags} -s pwsh | Out-String | Invoke-Expression -ErrorAction SilentlyContinue
            }}

            if (-not $__mise_pwsh_original_chpwd_function){{
                $Global:__mise_pwsh_original_chpwd_function=$ExecutionContext.SessionState.InvokeCommand.LocationChangedAction;

                if ($__mise_original_pwsh_chpwd_function) {{
                    $ExecutionContext.SessionState.InvokeCommand.LocationChangedAction = [Delegate]::Combine($__mise_pwsh_original_chpwd_function, {{_mise_hook}})
                }}
                else {{
                    $ExecutionContext.SessionState.InvokeCommand.LocationChangedAction = {{_mise_hook}}
                }}
            }}

            _mise_hook
            "#});
        out
    }

    fn deactivate(&self) -> String {
        formatdoc! {r#"
        $ExecutionContext.SessionState.InvokeCommand.LocationChangedAction = $__mise_pwsh_original_chpwd_function
        Remove-Variable __mise_pwsh_original_chpwd_function -ErrorAction SilentlyContinue
        Remove-Item function:_mise_hook -ErrorAction SilentlyContinue
        Remove-Item function:mise -ErrorAction SilentlyContinue
        $Env:MISE_SHELL=""
        $Env:__MISE_WATCH=""
        $Env:__MISE_DIFF=""
        "#}
    }

    fn set_env(&self, k: &str, v: &str) -> String {
        let k = powershell_escape(k.into());
        let v = powershell_escape(v.into());
        format!("$Env:{k}=\"{v}\"\n")
    }

    fn prepend_env(&self, k: &str, v: &str) -> String {
        let k = powershell_escape(k.into());
        let v = powershell_escape(v.into());
        format!("$Env:{k}=\"{v}$([IO.Path]::PathSeparator)\" + $env:{k}\n")
    }

    fn unset_env(&self, k: &str) -> String {
        let k = powershell_escape(k.into());
        format!("$Env:{k}=\"\"\n")
    }
}

impl Display for Pwsh {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "pwsh")
    }
}

#[cfg(test)]
mod tests {
    use insta::assert_snapshot;
    use test_log::test;

    use crate::test::replace_path;

    use super::*;

    #[test]
    fn test_activate() {
        let pwsh = Pwsh::default();
        let exe = Path::new("/some/dir/mise");
        assert_snapshot!(pwsh.activate(exe, " --status".into()));
    }

    #[test]
    fn test_set_env() {
        assert_snapshot!(Pwsh::default().set_env("FOO", "1"));
    }

    #[test]
    fn test_prepend_env() {
        let pwsh = Pwsh::default();
        assert_snapshot!(replace_path(&pwsh.prepend_env("PATH", "/some/dir:/2/dir")));
    }

    #[test]
    fn test_unset_env() {
        assert_snapshot!(Pwsh::default().unset_env("FOO"));
    }

    #[test]
    fn test_deactivate() {
        let deactivate = Pwsh::default().deactivate();
        assert_snapshot!(replace_path(&deactivate));
    }
}

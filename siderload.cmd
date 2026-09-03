@echo off
rem Convenience launcher for siderload.ps1 - lets it be run directly
rem (double-click, or `siderload.cmd` from any shell: cmd.exe, PowerShell,
rem or Git Bash) without needing an explicit
rem `powershell -ExecutionPolicy Bypass -File ...` invocation each time.
rem Running the bare .ps1 path directly does NOT work reliably - depending
rem on the shell/handler it goes through, it either tries (and fails) to
rem find a `pwsh` file association, or gets executed as if it were a plain
rem shell script and chokes on line 1's `<#` (PowerShell's comment syntax) -
rem in both cases with no visible error if launched by double-click, which
rem just looks like "nothing happened."
rem -ExecutionPolicy Bypass here only applies to this one process - it does
rem not change any persistent Windows/PowerShell setting.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0siderload.ps1" %*

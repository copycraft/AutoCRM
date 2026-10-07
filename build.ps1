# Runs build.sh through Git Bash on Windows. Not whatever `bash` is first on the PATH: that is
# often WSL's launcher, which cannot open a Windows path and would build with WSL's toolchain.
param([Parameter(ValueFromRemainingArguments)] $BuildArgs)
$candidates = @()
$git = Get-Command git -ErrorAction SilentlyContinue
if ($git) { $candidates += Join-Path (Split-Path (Split-Path $git.Source)) 'bin\bash.exe' }
$candidates += Join-Path $env:ProgramFiles 'Git\bin\bash.exe'
$bash = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $bash) { throw "Git Bash not found; install Git for Windows" }
& $bash (($PSScriptRoot -replace '\\', '/') + '/build.sh') @BuildArgs
exit $LASTEXITCODE

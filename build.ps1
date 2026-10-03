# Runs build.sh through Git Bash (or WSL bash) on Windows.
param([Parameter(ValueFromRemainingArguments)] $BuildArgs)
$bash = (Get-Command bash -ErrorAction SilentlyContinue).Source
if (-not $bash) { throw "bash not found; install Git for Windows" }
& $bash "$PSScriptRoot/build.sh" @Args
exit $LASTEXITCODE

param(
  [string]$Manifest = "examples/plugin_example/Cargo.toml",
  [string]$Features = "plugins,json",
  [string]$Profile = "debug"
)

Set-StrictMode -Version Latest

Write-Host "Building plugin using manifest: $Manifest"

$build = cargo build --manifest-path $Manifest
if ($LASTEXITCODE -ne 0) {
  Write-Error "Failed to build plugin (cargo build returned exit code $LASTEXITCODE)"
  exit $LASTEXITCODE
}

# Determine expected artifact names for the current platform
$candidates = @()
if ($IsWindows) {
  $candidates += "examples/plugin_example/target/$Profile/plugin_example.dll"
  $candidates += "target/$Profile/plugin_example.dll"
}
elseif ($IsMacOS) {
  $candidates += "examples/plugin_example/target/$Profile/libplugin_example.dylib"
  $candidates += "target/$Profile/libplugin_example.dylib"
}
else {
  $candidates += "examples/plugin_example/target/$Profile/libplugin_example.so"
  $candidates += "target/$Profile/libplugin_example.so"
}

$artifact = $null
foreach ($c in $candidates) {
  if (Test-Path $c) { $artifact = (Resolve-Path $c).Path; break }
}

if (-not $artifact) {
  Write-Error "Built plugin but could not find compiled artifact. Searched: $($candidates -join ', ')"
  Write-Error "You may need to adjust this script to match your cargo target directory layout."
  exit 2
}

Write-Host "Found plugin artifact at: $artifact"

Write-Host "Running plugin_loader test (features: $Features)"

$testCmd = "cargo test --no-default-features --features `"$Features`" --test plugin_loader"
Write-Host "=> $testCmd"

Invoke-Expression $testCmd
exit $LASTEXITCODE

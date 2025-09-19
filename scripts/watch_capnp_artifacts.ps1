# Poll GitHub Actions for capnpc_diagnostics artifacts for branch ci/pin-capnp-workflows
# Requires gh CLI authenticated and jq available (gh has built-in jq-style filtering via --jq/--json)

param(
  [int]$intervalSeconds = 30,
  [int]$maxChecks = 120
)

$repo = "ciresnave/commy"
$branch = "ci/pin-capnp-workflows"

Write-Host "Watching GitHub Actions for artifacts named 'capnpc_diagnostics' on branch $branch"
for ($i = 0; $i -lt $maxChecks; $i++) {
  Write-Host "Check $($i+1)/$maxChecks..."
  # List artifacts and filter by name
  $artifacts = gh api repos/$repo/actions/artifacts --jq '.artifacts[] | select(.name | test("capnpc_diagnostics"))'
  if ($LASTEXITCODE -ne 0) {
    Write-Host "gh api failed with exit code $LASTEXITCODE"
    Start-Sleep -Seconds $intervalSeconds
    continue
  }
  if ([string]::IsNullOrWhiteSpace($artifacts)) {
    Write-Host "No capnpc_diagnostics artifacts found yet."
  }
  else {
    Write-Host "Found artifacts:"
    $list = gh api repos/$repo/actions/artifacts --jq '.artifacts[] | select(.name | test("capnpc_diagnostics")) | {id: .id, name: .name, created_at: .created_at}'
    Write-Host $list
    # Download each artifact
    $ids = gh api repos/$repo/actions/artifacts --jq '.artifacts[] | select(.name | test("capnpc_diagnostics")) | .id'
    foreach ($id in $ids) {
      $idTrim = $id -replace '"', ''
      Write-Host "Downloading artifact id $idTrim..."
      gh api repos/$repo/actions/artifacts/$idTrim/zip -H "Accept: application/vnd.github+json" --output "artifact_$idTrim.zip"
      if ($LASTEXITCODE -eq 0) {
        Write-Host "Downloaded artifact_$idTrim.zip"
        # Unzip into diagnostics/artifact_$id
        $dest = "capnpc_diagnostics_artifacts/artifact_$idTrim"
        New-Item -ItemType Directory -Force -Path $dest | Out-Null
        Expand-Archive -LiteralPath "artifact_$idTrim.zip" -DestinationPath $dest -Force
        Write-Host "Extracted to $dest"
      }
      else {
        Write-Host "Failed to download artifact $idTrim"
      }
    }
    Write-Host "Artifacts downloaded. Exiting watcher."
    break
  }
  Start-Sleep -Seconds $intervalSeconds
}
Write-Host "Watcher finished."

$runId = 17799343414
$repo = 'ciresnave/commy'
while ($true) {
  $raw = gh run view $runId --repo $repo --json status --jq '.status' 2>$null
  if ($raw -eq $null) {
    Write-Host "gh returned empty status, retrying..."
  }
  else {
    $status = $raw.Trim()
    Write-Host "Run status: '$status'"
    if ($status -eq 'completed') {
      Write-Host "Run completed; downloading logs"
      gh run download $runId --repo $repo --dir gh-logs/run-$runId
      break
    }
  }
  Start-Sleep -Seconds 6
}
Write-Host "Done"

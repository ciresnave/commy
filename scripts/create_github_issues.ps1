<#
Create GitHub issues from the prepared list in docs/issues_from_proposed_improvements.md

NOTE: This script is a convenience tool for converting the `docs/issues_from_proposed_improvements.md`
items into GitHub issues. It is unrelated to the auth validation fix; consider moving this script and
the docs/ changes into a separate, focused pull request to keep reviews small and scoped.

Usage:
  - Export your GitHub token into environment variable GITHUB_TOKEN (recommended: a personal access token with repo scope)
    PowerShell: $env:GITHUB_TOKEN = '<token>'
  - Optionally set GITHUB_REPO to 'owner/repo' (defaults to 'ciresnave/commy')
  - Run: .\scripts\create_github_issues.ps1

This script will create one issue per item with a suggested title and body and will print created issue URLs.
Be careful: running this will create issues on the remote repo.
#>

param(
  [switch]$WhatIfMode
)

# Determine repository
$DefaultRepo = 'ciresnave/commy'
$Repo = $env:GITHUB_REPO
if (-not $Repo) {
    $Repo = $DefaultRepo
    Write-Warning "No GITHUB_REPO environment variable set. Using default repository: $DefaultRepo. This may create issues in the default repo."
    if ($WhatIfMode) {
        Write-Host "[DRY-RUN] Would use default repository: $DefaultRepo"
    }
} else {
    if ($WhatIfMode) {
        Write-Host "[DRY-RUN] Would use repository: $Repo"
    }
}

# If dry-run, print a summary and skip actual issue creation
if ($WhatIfMode) {
    Write-Host "[DRY-RUN] No issues will be created. All side effects are suppressed."
}

if (-not $env:GITHUB_TOKEN) {
  Write-Error "GITHUB_TOKEN environment variable not found. Set it to a Personal Access Token with 'repo' scope, e.g.:`n$env:GITHUB_TOKEN = '<token>'"
  exit 1
}

$repo = $env:GITHUB_REPO
if (-not $repo) { $repo = 'ciresnave/commy' }

Write-Host "Creating issues on repository: $repo"

# Retry/backoff settings
$maxAttempts = 5
$baseDelaySeconds = 1


$issues = @(
  @{ title = 'Add property-based tests for manager invariants'; body = "Add proptest suites for core manager invariants such as file ID allocation and reuse, concurrent creates, and behavior under race conditions. Reference: docs/issues_from_proposed_improvements.md"; labels = @('test', 'enhancement') },
  @{ title = 'Implement a simple object pool for frequently allocated structures'; body = "Add a small object pool utility (zero-allocation where possible) for frequently allocated items such as buffers and transport objects. Include benchmarks and tests. Reference: docs/issues_from_proposed_improvements.md"; labels = @('performance', 'enhancement') },
  @{ title = 'Strictly use AuthProvider and add integration tests'; body = "Ensure SharedFileManager only depends on the AuthProvider abstraction; add end-to-end integration tests that exercise RealAuthProvider against a running auth-framework instance. Reference: docs/auth-refactor.md"; labels = @('security', 'test') },
  @{ title = 'Inventory and implement test doubles (stubs/mocks)'; body = "Create a catalog of test doubles for network, storage, and FFI layers and provide implementations for unit/integration tests. Reference: PROPOSED_IMPROVEMENTS.md"; labels = @('test', 'chore') },
  @{ title = 'Add CI jobs for manager feature and proptest suites'; body = "Add targeted CI workflows that run the manager feature tests (unit + proptest) and ensure fast feedback. Limit proptest runs under time budgets or use shrink settings. Reference: PROPOSED_IMPROVEMENTS.md"; labels = @('ci', 'infra') },
  @{ title = 'Documentation: expand auth-refactor and migration guide'; body = "Expand docs/auth-refactor.md and provide a migration guide for plugin and FFI changes introduced by the auth provider refactor. Include examples and upgrade notes. Reference: PROPOSED_IMPROVEMENTS.md"; labels = @('docs') }
)

foreach ($issue in $issues) {
  $payload = @{ title = $issue.title; body = $issue.body; labels = $issue.labels } | ConvertTo-Json -Depth 4
  Write-Host "Preparing issue: $($issue.title)"
  if ($WhatIfMode) {
    Write-Host "WhatIf: Would POST to https://api.github.com/repos/$repo/issues with payload:`n$payload`n"
    continue
  }

  $attempt = 0
  while ($true) {
    $attempt++
    try {
      $response = Invoke-RestMethod -Method Post -Uri "https://api.github.com/repos/$repo/issues" -Headers @{ Authorization = "token $env:GITHUB_TOKEN"; Accept = 'application/vnd.github.v3+json' } -Body $payload -ContentType 'application/json'
      if ($response -and $response.html_url) {
        Write-Host "Created: $($response.html_url)"
        break
      }
      else {
        Write-Warning "Unexpected response while creating issue: $($response | ConvertTo-Json -Depth 3)"
        # Treat unexpected response as non-retryable
        break
      }
    }
    catch {
      $ex = $_.Exception
      $status = $null
      if ($ex.Response -is [System.Net.HttpWebResponse]) {
        $http = $ex.Response
        $status = [int]$http.StatusCode
      }

      # If rate limited (429) or transient network error, retry with backoff
      if ($status -eq 429 -or $ex -is [System.Net.WebException]) {
        if ($attempt -ge $maxAttempts) {
          Write-Error "Failed to create issue '$($issue.title)' after $attempt attempts: $($_.Exception.Message)"
          break
        }

        # Determine wait time: exponential backoff with jitter
        $delay = [math]::Pow(2, $attempt - 1) * $baseDelaySeconds
        # Add small jitter (0..0.5s)
        $jitter = (Get-Random -Minimum 0 -Maximum 0.5)
        $wait = [math]::Round($delay + $jitter, 2)
        Write-Warning "Transient error creating issue '$($issue.title)'. Attempt $attempt/$maxAttempts. Waiting $wait seconds before retrying..."
        Start-Sleep -Seconds $wait
        continue
      }

      # Non-retryable: map common statuses
      if ($status -eq 401) { Write-Error "Authentication error (401) when creating issue '$($issue.title)'. Check GITHUB_TOKEN permissions."; break }
      if ($status -eq 403) { Write-Error "Forbidden (403) when creating issue '$($issue.title)'. You may lack repository permissions or be rate-limited."; break }
      Write-Warning "Error creating issue '$($issue.title)': $($_.Exception.Message)"
      break
    }
  }
}

Write-Host "Done."

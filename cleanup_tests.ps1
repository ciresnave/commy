#!/usr/bin/env pwsh
#
# Test Cleanup Script for Commy Project
# Removes test artifacts and temporary files left by tests
#

Write-Host "🧹 Commy Test Cleanup Script" -ForegroundColor Cyan
Write-Host "=============================`n" -ForegroundColor Cyan

$RootPath = $PSScriptRoot
if (-not $RootPath) {
  $RootPath = Get-Location
}

Write-Host "Cleaning up test artifacts in: $RootPath" -ForegroundColor Yellow

# Patterns to clean up
$CleanupPatterns = @(
  # ThreadId test directories
  "test_files_ThreadId*",

  # Temporary test directories
  "temp",
  "test_integration_files",
  "test_phase2_cleanup",

  # Profiling data files
  "*.profraw",

  # Memory-mapped test files in root
  "*.mmap",
  "*.bin",

  # Test database files
  "test*.sqlite*",
  "db.sqlite*"
)

$CleanedCount = 0

foreach ($Pattern in $CleanupPatterns) {
  $FullPattern = Join-Path $RootPath $Pattern
  $Items = Get-ChildItem -Path $FullPattern -ErrorAction SilentlyContinue

  foreach ($Item in $Items) {
    try {
      if ($Item.PSIsContainer) {
        Write-Host "🗂️  Removing directory: $($Item.Name)" -ForegroundColor Red
        Remove-Item -Path $Item.FullName -Recurse -Force
      }
      else {
        Write-Host "🗄️  Removing file: $($Item.Name)" -ForegroundColor Red
        Remove-Item -Path $Item.FullName -Force
      }
      $CleanedCount++
    }
    catch {
      Write-Warning "Failed to remove $($Item.FullName): $($_.Exception.Message)"
    }
  }
}

# Clean up specific test directories that should be preserved but cleaned
$TestDirsToClean = @(
  "test_files",
  "test_files_shared",
  "test_phase2_edge_cases",
  "test_phase2_files",
  "test_phase2_multiple_files",
  "test_phase2_sizes"
)

foreach ($Dir in $TestDirsToClean) {
  $DirPath = Join-Path $RootPath $Dir
  if (Test-Path $DirPath) {
    Write-Host "🧽 Cleaning contents of: $Dir" -ForegroundColor Yellow

    # Remove .mmap files and persistence subdirectories
    $MmapFiles = Get-ChildItem -Path $DirPath -Filter "*.mmap" -ErrorAction SilentlyContinue
    foreach ($File in $MmapFiles) {
      Remove-Item -Path $File.FullName -Force
      $CleanedCount++
    }

    $PersistenceDir = Join-Path $DirPath "persistence"
    if (Test-Path $PersistenceDir) {
      Remove-Item -Path $PersistenceDir -Recurse -Force -ErrorAction SilentlyContinue
      $CleanedCount++
    }
  }
}

Write-Host "`n✅ Cleanup complete!" -ForegroundColor Green
Write-Host "📊 Removed $CleanedCount items" -ForegroundColor Green

# Suggest .gitignore additions
Write-Host "`n💡 Consider adding these patterns to .gitignore:" -ForegroundColor Cyan
Write-Host "   *.profraw" -ForegroundColor Gray
Write-Host "   *.mmap" -ForegroundColor Gray
Write-Host "   test_files_ThreadId*/" -ForegroundColor Gray
Write-Host "   temp/" -ForegroundColor Gray
Write-Host "   test_integration_files/" -ForegroundColor Gray
Write-Host "   test_*.sqlite*" -ForegroundColor Gray
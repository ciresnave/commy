$log = 'gh-logs/run-17799343414.log'
if (!(Test-Path $log)) { Write-Host "Log file not found: $log"; exit 1 }
$patterns = @(
  'capnp codegen succeeded',
  'found generated file',
  'moved generated',
  "couldn't read",
  'include!(concat!(env!("OUT_DIR"), "/example_capnp.rs"))',
  'schema-preview',
  'copied schema:'
)
foreach ($pat in $patterns) {
  Write-Host "\n=== Matches for: '$pat' ==="
  $matches = Select-String -Path $log -Pattern $pat -SimpleMatch -CaseSensitive:$false -Context 3, 1
  if ($matches) {
    foreach ($m in $matches) {
      Write-Host "Line $($m.LineNumber): $($m.Line)"
      if ($m.Context.PreContext) { foreach ($l in $m.Context.PreContext) { Write-Host "  PRE: $l" } }
      if ($m.Context.PostContext) { foreach ($l in $m.Context.PostContext) { Write-Host "  POST: $l" } }
    }
  }
  else { Write-Host "(no matches)" }
}
Write-Host "\nScan complete."
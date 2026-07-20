param(
    [Parameter(Mandatory, Position = 0)]
    [string]$Message
)

$ErrorActionPreference = "Stop"

git add -A
git commit -m $Message
git push origin master

if ($LASTEXITCODE -eq 0) {
    Write-Host "`nPushed successfully." -ForegroundColor Green
}

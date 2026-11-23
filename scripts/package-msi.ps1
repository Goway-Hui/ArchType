# 打 Windows 安装包（MSI）：先编 TSF DLL 与 ArchType.exe，再用 WiX v4 打包。
#
# 用法（在 Windows runner 上）：
#   scripts/package-msi.ps1 -Version 0.8.0 -DataDir dist/data -OutDir dist/windows
# 需要：Rust（x86_64-pc-windows-msvc）、WiX 4（dotnet tool install --global wix）。
param(
    [Parameter(Mandatory = $true)][string]$Version,
    [Parameter(Mandatory = $true)][string]$DataDir,
    [string]$OutDir = "dist/windows"
)
$ErrorActionPreference = "Stop"

if (-not (Test-Path $DataDir)) { throw "找不到数据目录 $DataDir（CI 的 data 任务会产出）" }
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

cargo build --release --manifest-path win-rs/Cargo.toml
if ($LASTEXITCODE -ne 0) { throw "win-rs 编译失败" }

$source = (Resolve-Path "win-rs/target/release").Path
$data = (Resolve-Path $DataDir).Path
$msi = Join-Path (Resolve-Path $OutDir).Path "ArchType-$Version-x64.msi"

wix build -arch x64 -o $msi `
    -d Version=$Version `
    -d SourceDir=$source `
    -d DataDir=$data `
    -d RepoDir=(Get-Location).Path `
    windows/ArchType.wxs
if ($LASTEXITCODE -ne 0) { throw "wix build 失败" }

Write-Output "MSI: $msi"

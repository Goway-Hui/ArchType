# 打 Windows 安装包（MSI）：先编 TSF DLL 与 ArchType.exe，再用 WiX v4 打包。
#
# 用法（在 Windows runner 上）：
#   scripts/package-msi.ps1 -Version 0.8.0 -DataDir dist/data -OutDir dist/windows
# 需要：Rust（x86_64-pc-windows-msvc）、WiX 4（dotnet tool install --global wix）。
param(
    [Parameter(Mandatory = $true)][string]$Version,
    [Parameter(Mandatory = $true)][string]$DataDir,
    # 32 位 DLL（i686-pc-windows-msvc 的 release 目录）；不给就只装 64 位
    [string]$X86Dir = "",
    # nsh 打出来的设置界面 exe；不给就不带设置界面
    [string]$SettingsExe = "",
    [string]$OutDir = "dist/windows"
)
$ErrorActionPreference = "Stop"

if (-not (Test-Path $DataDir)) { throw "找不到数据目录 $DataDir（CI 的 data 任务会产出）" }
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null

cargo build --release --manifest-path win-rs/Cargo.toml
if ($LASTEXITCODE -ne 0) { throw "win-rs 编译失败" }

$source = (Resolve-Path "win-rs/target/release").Path
$data = (Resolve-Path $DataDir).Path
$repo = (Get-Location).Path
$wixArgs = @()
if ($X86Dir -ne "") {
    if (-not (Test-Path "$X86Dir\archtype_tsf.dll")) { throw "32 位 DLL 不在 $X86Dir" }
    $wixArgs += @("-d", "X86Dir=$((Resolve-Path $X86Dir).Path)")
} else {
    # 没给 32 位：让 .wxs 里 32 位组件用 64 位 DLL（构建仍通过，但不注册 32 位视图时会有问题）
    throw "必须提供 32 位 DLL（-X86Dir）；Windows 的 32 位应用需要它"
}
if ($SettingsExe -ne "") {
    if (-not (Test-Path $SettingsExe)) { throw "设置界面 exe 不存在：$SettingsExe" }
    $wixArgs += @("-d", "SettingsExe=$((Resolve-Path $SettingsExe).Path)")
} else {
    $wixArgs += @("-d", "SettingsExe=")
}
$msi = Join-Path (Resolve-Path $OutDir).Path "ArchType-$Version-x64.msi"

# 每个 -d 的值都要单独引用：路径里有空格、或者写成 (Get-Location).Path 时，
# PowerShell 会把括号当字面量传给 wix，wix 就把它当成输入文件了
wix build -arch x64 -o "$msi" `
    -d "Version=$Version" `
    -d "SourceDir=$source" `
    -d "DataDir=$data" `
    -d "RepoDir=$repo" `
    $wixArgs `
    "windows/ArchType.wxs"
if ($LASTEXITCODE -ne 0) { throw "wix build 失败" }

Write-Output "MSI: $msi"

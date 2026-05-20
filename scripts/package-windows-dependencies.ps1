param(
  [string]$ImageMagickPath = "C:\Program Files\ImageMagick-7.1.2-Q8",
  [string]$PopplerPath = "C:\popper",
  [string]$OnnxRuntimeDir = "$PSScriptRoot\..\src-tauri\resources\win-x86_64",
  [string]$OutputPath = "$PSScriptRoot\..\dist\invoicevault-dependencies-win-x86_64.zip"
)

$ErrorActionPreference = "Stop"

function Resolve-RequiredPath {
  param(
    [string]$Path,
    [string]$Name
  )

  $resolved = Resolve-Path -LiteralPath $Path -ErrorAction SilentlyContinue
  if (-not $resolved) {
    throw "$Name not found: $Path"
  }
  return $resolved.Path
}

$imageMagick = Resolve-RequiredPath -Path $ImageMagickPath -Name "ImageMagick"
$poppler = Resolve-RequiredPath -Path $PopplerPath -Name "Poppler"
$onnxRuntimeDir = Resolve-RequiredPath -Path $OnnxRuntimeDir -Name "ONNX Runtime directory"
$onnxRuntimeDll = Join-Path $onnxRuntimeDir "onnxruntime.dll"
if (-not (Test-Path -LiteralPath $onnxRuntimeDll)) {
  throw "onnxruntime.dll not found: $onnxRuntimeDll"
}

$stagingRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("invoicevault-deps-" + [System.Guid]::NewGuid().ToString("N"))
$stagingDeps = Join-Path $stagingRoot "win-x86_64"
$null = New-Item -ItemType Directory -Path $stagingDeps -Force

try {
  Copy-Item -LiteralPath $onnxRuntimeDll -Destination (Join-Path $stagingDeps "onnxruntime.dll") -Force
  Copy-Item -LiteralPath $imageMagick -Destination (Join-Path $stagingDeps "ImageMagick") -Recurse -Force
  Copy-Item -LiteralPath $poppler -Destination (Join-Path $stagingDeps "poppler") -Recurse -Force

  $outputFullPath = [System.IO.Path]::GetFullPath($OutputPath)
  $outputDir = Split-Path -Parent $outputFullPath
  $null = New-Item -ItemType Directory -Path $outputDir -Force
  if (Test-Path -LiteralPath $outputFullPath) {
    Remove-Item -LiteralPath $outputFullPath -Force
  }

  Compress-Archive -Path (Join-Path $stagingRoot "win-x86_64") -DestinationPath $outputFullPath -CompressionLevel Optimal
  Write-Host "Created $outputFullPath"
} finally {
  Remove-Item -LiteralPath $stagingRoot -Recurse -Force -ErrorAction SilentlyContinue
}

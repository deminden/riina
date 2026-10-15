[CmdletBinding()]
param(
    [string]$OutputPath = (Join-Path $env:TEMP ("riina-desktop-{0}.png" -f (Get-Date -Format 'yyyyMMdd-HHmmss')))
)

$ErrorActionPreference = 'Stop'

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;

namespace RiinaDesktopCapture {
    public static class Dpi {
        private static readonly IntPtr PerMonitorV2 = new IntPtr(-4);

        [DllImport("user32.dll", SetLastError = true)]
        private static extern bool SetProcessDpiAwarenessContext(IntPtr value);

        [DllImport("user32.dll", SetLastError = true)]
        private static extern bool SetProcessDPIAware();

        public static bool Enable() {
            return SetProcessDpiAwarenessContext(PerMonitorV2) || SetProcessDPIAware();
        }
    }
}
'@

# Set awareness before WinForms resolves virtual-screen bounds, otherwise a
# high-DPI desktop can be captured as only the process's scaled top-left region.
[void][RiinaDesktopCapture.Dpi]::Enable()

$bounds = [System.Windows.Forms.SystemInformation]::VirtualScreen
if ($bounds.Width -le 0 -or $bounds.Height -le 0) {
    throw "Windows returned invalid virtual-desktop bounds: $($bounds.Width)x$($bounds.Height)."
}

$fullOutputPath = [System.IO.Path]::GetFullPath($OutputPath)
$directory = [System.IO.Path]::GetDirectoryName($fullOutputPath)
if (-not [string]::IsNullOrWhiteSpace($directory)) {
    [void][System.IO.Directory]::CreateDirectory($directory)
}

$bitmap = [System.Drawing.Bitmap]::new($bounds.Width, $bounds.Height)
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$hasVisibleContent = $false
try {
    $graphics.CopyFromScreen(
        $bounds.Left,
        $bounds.Top,
        0,
        0,
        $bitmap.Size,
        [System.Drawing.CopyPixelOperation]::SourceCopy
    )
    $bitmap.Save($fullOutputPath, [System.Drawing.Imaging.ImageFormat]::Png)

    # A successful GDI call can still return a uniformly black desktop when
    # the capture bridge cannot read the current compositor surface.
    for ($gridY = 0; $gridY -lt 5 -and -not $hasVisibleContent; $gridY++) {
        for ($gridX = 0; $gridX -lt 5; $gridX++) {
            $x = [int](($bounds.Width - 1) * ($gridX + 0.5) / 5)
            $y = [int](($bounds.Height - 1) * ($gridY + 0.5) / 5)
            $pixel = $bitmap.GetPixel($x, $y)
            if ($pixel.R -gt 8 -or $pixel.G -gt 8 -or $pixel.B -gt 8) {
                $hasVisibleContent = $true
                break
            }
        }
    }
}
finally {
    $graphics.Dispose()
    $bitmap.Dispose()
}

if (-not $hasVisibleContent) {
    Write-Warning "The saved desktop image is black at all 25 probe points; treat this capture as invalid visual evidence."
}

[pscustomobject]@{
    Path               = $fullOutputPath
    Width              = $bounds.Width
    Height             = $bounds.Height
    Bytes              = (Get-Item -LiteralPath $fullOutputPath).Length
    HasNonBlackProbe   = $hasVisibleContent
}

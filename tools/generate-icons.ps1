param(
    [string]$AssetsDirectory = (Join-Path $PSScriptRoot '..\assets')
)

# Draws the "딮" icon in the Galmuri14 pixel font on a transparent background:
# - icon-light.png / tray-light-<size>.png: dark glyph for light themes
# - icon-dark.png / tray-dark-<size>.png: light glyph for dark themes
# - app.ico: dark glyph with a one-pixel light outline, readable on any background,
#   because Windows cannot switch an executable's icon with the theme.
# Galmuri14 (https://github.com/quiple/galmuri, SIL OFL 1.1) must be installed.

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

$Glyph = '딮'
$GlyphFont = 'Galmuri14 Regular'
# The font's native pixel size, where one font pixel is one screen pixel.
$GlyphPixelSize = 15
$DarkGlyph = [System.Drawing.Color]::FromArgb(255, 15, 43, 70)
$LightGlyph = [System.Drawing.Color]::FromArgb(255, 255, 255, 255)
$Outline = [System.Drawing.Color]::FromArgb(255, 255, 255, 255)
# Keep in sync with the tray icon choice in ui/app.slint.
$TraySizes = @(16, 20, 24, 32, 40, 48)

# The glyph as rows of booleans, one per font pixel, trimmed to its bounds.
function Get-GlyphMask {
    $canvas = $GlyphPixelSize * 2
    $family = [System.Drawing.FontFamily]::new($GlyphFont)
    $font = [System.Drawing.Font]::new($family, $GlyphPixelSize, [System.Drawing.FontStyle]::Regular, [System.Drawing.GraphicsUnit]::Pixel)
    $bitmap = [System.Drawing.Bitmap]::new($canvas, $canvas, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
        # Rendered without smoothing so each font pixel comes out as one solid pixel.
        $graphics.Clear([System.Drawing.Color]::White)
        $graphics.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::SingleBitPerPixel
        $graphics.DrawString($Glyph, $font, [System.Drawing.Brushes]::Black, 0, 0, [System.Drawing.StringFormat]::GenericTypographic)
    }
    finally {
        $graphics.Dispose()
        $font.Dispose()
        $family.Dispose()
    }

    try {
        $left = $canvas; $top = $canvas; $right = -1; $bottom = -1
        for ($y = 0; $y -lt $canvas; $y++) {
            for ($x = 0; $x -lt $canvas; $x++) {
                if ($bitmap.GetPixel($x, $y).R -lt 128) {
                    $left = [Math]::Min($left, $x); $right = [Math]::Max($right, $x)
                    $top = [Math]::Min($top, $y); $bottom = [Math]::Max($bottom, $y)
                }
            }
        }
        if ($right -lt 0) {
            throw "$GlyphFont is not installed or has no '$Glyph' glyph."
        }

        $mask = [bool[, ]]::new($bottom - $top + 1, $right - $left + 1)
        for ($y = $top; $y -le $bottom; $y++) {
            for ($x = $left; $x -le $right; $x++) {
                $mask[($y - $top), ($x - $left)] = $bitmap.GetPixel($x, $y).R -lt 128
            }
        }
        return , $mask
    }
    finally {
        $bitmap.Dispose()
    }
}

# The mask grown by one pixel in every direction, including diagonals.
function Get-OutlineMask {
    param([bool[, ]]$Mask)

    $height = $Mask.GetLength(0) + 2
    $width = $Mask.GetLength(1) + 2
    $outline = [bool[, ]]::new($height, $width)
    for ($y = 0; $y -lt $Mask.GetLength(0); $y++) {
        for ($x = 0; $x -lt $Mask.GetLength(1); $x++) {
            if (-not $Mask[$y, $x]) { continue }
            for ($dy = 0; $dy -le 2; $dy++) {
                for ($dx = 0; $dx -le 2; $dx++) {
                    $outline[($y + $dy), ($x + $dx)] = $true
                }
            }
        }
    }
    return , $outline
}

# Layers are drawn in order, each a mask and a color. The first layer fills the
# square; font pixels become antialiased squares, so sizes that are not a whole
# multiple of the font still get evenly weighted strokes.
function Get-IconPngBytes {
    param([int]$Size, [object[]]$Layers)

    $base = $Layers[0].Mask
    $scale = $Size / [Math]::Max($base.GetLength(0), $base.GetLength(1))
    $bitmap = [System.Drawing.Bitmap]::new($Size, $Size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try {
        $graphics.Clear([System.Drawing.Color]::Transparent)
        $graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
        $graphics.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
        foreach ($layer in $Layers) {
            $mask = $layer.Mask
            $offsetX = ($Size - $mask.GetLength(1) * $scale) / 2
            $offsetY = ($Size - $mask.GetLength(0) * $scale) / 2
            # One path for the whole layer, so neighboring squares leave no seams.
            $path = [System.Drawing.Drawing2D.GraphicsPath]::new([System.Drawing.Drawing2D.FillMode]::Winding)
            $brush = [System.Drawing.SolidBrush]::new($layer.Color)
            try {
                for ($y = 0; $y -lt $mask.GetLength(0); $y++) {
                    for ($x = 0; $x -lt $mask.GetLength(1); $x++) {
                        if ($mask[$y, $x]) {
                            $path.AddRectangle([System.Drawing.RectangleF]::new(
                                $offsetX + $x * $scale, $offsetY + $y * $scale, $scale, $scale))
                        }
                    }
                }
                $graphics.FillPath($brush, $path)
            }
            finally {
                $brush.Dispose()
                $path.Dispose()
            }
        }

        $stream = [System.IO.MemoryStream]::new()
        try {
            $bitmap.Save($stream, [System.Drawing.Imaging.ImageFormat]::Png)
            return , $stream.ToArray()
        }
        finally {
            $stream.Dispose()
        }
    }
    finally {
        $graphics.Dispose()
        $bitmap.Dispose()
    }
}

function Write-PngIconFile {
    param([object[]]$Layers, [int[]]$Sizes, [string]$Path)

    $images = [System.Collections.Generic.List[byte[]]]::new()
    foreach ($size in $Sizes) {
        $images.Add([byte[]](Get-IconPngBytes -Size $size -Layers $Layers))
    }
    $stream = [System.IO.File]::Create($Path)
    $writer = [System.IO.BinaryWriter]::new($stream)
    try {
        $writer.Write([uint16]0)
        $writer.Write([uint16]1)
        $writer.Write([uint16]$Sizes.Count)

        $offset = 6 + 16 * $Sizes.Count
        for ($index = 0; $index -lt $Sizes.Count; $index++) {
            $size = $Sizes[$index]
            $writer.Write([byte]($(if ($size -ge 256) { 0 } else { $size })))
            $writer.Write([byte]($(if ($size -ge 256) { 0 } else { $size })))
            $writer.Write([byte]0)
            $writer.Write([byte]0)
            $writer.Write([uint16]1)
            $writer.Write([uint16]32)
            $writer.Write([uint32]($images[$index].Length))
            $writer.Write([uint32]$offset)
            $offset += $images[$index].Length
        }

        foreach ($image in $images) {
            $writer.Write($image)
        }
    }
    finally {
        $writer.Dispose()
        $stream.Dispose()
    }
}

function Save-Png {
    param([object[]]$Layers, [int]$Size, [string]$Name)

    $path = Join-Path $AssetsDirectory $Name
    [System.IO.File]::WriteAllBytes("$path.new", (Get-IconPngBytes -Size $Size -Layers $Layers))
    Move-Item -LiteralPath "$path.new" -Destination $path -Force
}

$glyphMask = Get-GlyphMask

foreach ($variant in @(
    @{ Suffix = 'light'; Color = $DarkGlyph },
    @{ Suffix = 'dark'; Color = $LightGlyph }
)) {
    $layers = @(@{ Mask = $glyphMask; Color = $variant.Color })
    Save-Png -Layers $layers -Size 256 -Name "icon-$($variant.Suffix).png"
    # Windows shows the tray icon at the image's own size and scales it poorly,
    # so there is one per small-icon size (100% to 300% display scale).
    foreach ($size in $TraySizes) {
        Save-Png -Layers $layers -Size $size -Name "tray-$($variant.Suffix)-$size.png"
    }
}

$icoPath = Join-Path $AssetsDirectory 'app.ico'
$icoLayers = @(
    @{ Mask = (Get-OutlineMask -Mask $glyphMask); Color = $Outline },
    @{ Mask = $glyphMask; Color = $DarkGlyph }
)
Write-PngIconFile -Layers $icoLayers -Sizes @(16, 20, 24, 32, 40, 48, 64, 128, 256) -Path "$icoPath.new"
Move-Item -LiteralPath "$icoPath.new" -Destination $icoPath -Force

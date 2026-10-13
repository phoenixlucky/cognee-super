$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$Root = Split-Path -Parent $PSScriptRoot
$IconDir = Join-Path $Root 'src-tauri\icons'
New-Item -ItemType Directory -Force -Path $IconDir | Out-Null
$Bitmap = [Drawing.Bitmap]::new(256, 256, [Drawing.Imaging.PixelFormat]::Format32bppArgb)
$Graphics = [Drawing.Graphics]::FromImage($Bitmap)
$Graphics.SmoothingMode = [Drawing.Drawing2D.SmoothingMode]::AntiAlias
$Graphics.Clear([Drawing.Color]::Transparent)
$Path = [Drawing.Drawing2D.GraphicsPath]::new()
$Path.AddArc(14, 14, 228, 228, 180, 90)
$Path.AddArc(14, 14, 228, 228, 270, 90)
$Path.AddArc(14, 14, 228, 228, 0, 90)
$Path.AddArc(14, 14, 228, 228, 90, 90)
$Path.CloseFigure()
$Brush = [Drawing.Drawing2D.LinearGradientBrush]::new([Drawing.Rectangle]::new(0, 0, 256, 256), [Drawing.Color]::FromArgb(255, 22, 174, 154), [Drawing.Color]::FromArgb(255, 4, 100, 104), 35)
$Graphics.FillPath($Brush, $Path)
$Font = [Drawing.Font]::new('Segoe UI', 158, [Drawing.FontStyle]::Bold, [Drawing.GraphicsUnit]::Pixel)
$Format = [Drawing.StringFormat]::new()
$Format.Alignment = [Drawing.StringAlignment]::Center
$Format.LineAlignment = [Drawing.StringAlignment]::Center
$Graphics.DrawString('C', $Font, [Drawing.Brushes]::White, [Drawing.RectangleF]::new(18, 8, 220, 228), $Format)
$PngPath = Join-Path $IconDir 'icon.png'
$IcoPath = Join-Path $IconDir 'icon.ico'
$Bitmap.Save($PngPath, [Drawing.Imaging.ImageFormat]::Png)
$Stream = [IO.MemoryStream]::new()
$Bitmap.Save($Stream, [Drawing.Imaging.ImageFormat]::Png)
$Png = $Stream.ToArray()
$File = [IO.File]::Create($IcoPath)
$Writer = [IO.BinaryWriter]::new($File)
$Writer.Write([UInt16]0); $Writer.Write([UInt16]1); $Writer.Write([UInt16]1)
$Writer.Write([byte]0); $Writer.Write([byte]0); $Writer.Write([byte]0); $Writer.Write([byte]0)
$Writer.Write([UInt16]1); $Writer.Write([UInt16]32); $Writer.Write([UInt32]$Png.Length); $Writer.Write([UInt32]22); $Writer.Write($Png)
$Writer.Dispose(); $File.Dispose(); $Stream.Dispose(); $Graphics.Dispose(); $Bitmap.Dispose(); $Brush.Dispose(); $Path.Dispose(); $Font.Dispose(); $Format.Dispose()

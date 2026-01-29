# Agentreplay Desktop Icons

This directory contains the application icons for the Agentreplay Tauri desktop app.

## Source File

- **`desktop_icon.svg`** - The master source file for all icons (512x512)
  - Blue gradient background (#3b82f6 to #1d4ed8)
  - White nodes connected in a flow graph pattern
  - Represents the Agentreplay concept of tracking agent execution flows

## Generated Icons

The following icon files are generated from `desktop_icon.svg`:

- `32x32.png` - Small icon (Windows, Linux)
- `128x128.png` - Medium icon (macOS, Windows, Linux)
- `128x128@2x.png` - Retina medium icon (macOS)
- `icon.png` - Large icon (256x256)
- `icon.icns` - macOS icon bundle
- `icon.ico` - Windows icon

## Regenerating Icons

If you modify `desktop_icon.svg`, regenerate all icon formats by running:

```bash
# Method 1: Using Python script (recommended)
cd agentreplay-tauri/icons
python3 convert_svg_icons.py

# Method 2: Using shell script
chmod +x convert_icons.sh
./convert_icons.sh
```

### Requirements

The conversion script requires:
- Python 3.x
- cairosvg (`pip3 install --break-system-packages cairosvg`)
- Pillow (`pip3 install --break-system-packages Pillow`)

### Manual Conversion

If you don't have the required tools, you can:

1. **Use an online converter**: https://cloudconvert.com/svg-to-png
   - Export PNG at sizes: 32x32, 128x128, 256x256
   
2. **Use design tools**: Figma, Sketch, Adobe Illustrator
   - Export as PNG at the required sizes
   
3. **Use macOS `sips`** to convert to ICNS:
   ```bash
   sips -s format icns icon.png --out icon.icns
   ```

## Customizing the Icon

To create your own icon:

1. Edit `desktop_icon.svg` using:
   - Any SVG editor (Inkscape, Figma, Illustrator)
   - Text editor (it's just XML)
   
2. Keep the viewBox at "0 0 512 512" for best results

3. Use high contrast and simple shapes for small sizes

4. Run the conversion script to generate all formats

## Icon Configuration

The icons are configured in `tauri.conf.json`:

```json
{
  "bundle": {
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ]
  },
  "app": {
    "trayIcon": {
      "iconPath": "icons/icon.png"
    }
  }
}
```

## Notes

- The `.icns` file contains multiple resolutions for macOS
- The `.ico` file is used for Windows
- PNG files are used for Linux and as source for other formats
- All icons are generated with RGBA (with alpha channel) as required by Tauri

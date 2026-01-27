#!/usr/bin/env python3

# Copyright 2025 Sushanth (https://github.com/sushanthpy)
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

"""
Convert desktop_icon.svg to all required Tauri icon formats
Requires: cairosvg, Pillow
Install: pip3 install cairosvg Pillow
"""

import os
import sys

try:
    from cairosvg import svg2png
    from PIL import Image
    import io
except ImportError:
    print("ERROR: Required libraries not installed")
    print("Please run: pip3 install cairosvg Pillow")
    sys.exit(1)

def svg_to_png(svg_path, png_path, width, height):
    """Convert SVG to PNG at specified size"""
    svg2png(
        url=svg_path,
        write_to=png_path,
        output_width=width,
        output_height=height
    )
    print(f"✓ Created {png_path} ({width}x{height})")

def png_to_icns(png_path, icns_path):
    """Convert PNG to ICNS for macOS"""
    # Use sips command (macOS built-in)
    os.system(f'sips -s format icns "{png_path}" --out "{icns_path}" > /dev/null 2>&1')
    print(f"✓ Created {icns_path}")

def png_to_ico(png_32_path, ico_path):
    """Convert PNG to ICO for Windows"""
    img = Image.open(png_32_path)
    img.save(ico_path, format='ICO', sizes=[(32, 32)])
    print(f"✓ Created {ico_path}")

def main():
    # Get the directory of this script
    script_dir = os.path.dirname(os.path.abspath(__file__))
    svg_path = os.path.join(script_dir, 'desktop_icon.svg')
    
    if not os.path.exists(svg_path):
        print(f"ERROR: {svg_path} not found!")
        sys.exit(1)
    
    print("Converting desktop_icon.svg to Tauri icon formats...\n")
    
    # Generate PNG files at different sizes
    sizes = [
        (32, '32x32.png'),
        (128, '128x128.png'),
        (128, '128x128@2x.png'),
        (256, 'icon.png'),
    ]
    
    for size, filename in sizes:
        png_path = os.path.join(script_dir, filename)
        svg_to_png(svg_path, png_path, size, size)
    
    # Generate ICNS for macOS
    icon_png = os.path.join(script_dir, 'icon.png')
    icns_path = os.path.join(script_dir, 'icon.icns')
    png_to_icns(icon_png, icns_path)
    
    # Generate ICO for Windows
    png_32 = os.path.join(script_dir, '32x32.png')
    ico_path = os.path.join(script_dir, 'icon.ico')
    png_to_ico(png_32, ico_path)
    
    print("\n✅ All icons generated successfully!")
    print("Icons are ready for Tauri app")

if __name__ == '__main__':
    main()

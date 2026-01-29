#!/bin/bash

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

# Convert desktop_icon.svg to Tauri icons using macOS tools

set -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$SCRIPT_DIR"

SVG_FILE="desktop_icon.svg"

if [ ! -f "$SVG_FILE" ]; then
    echo "ERROR: $SVG_FILE not found!"
    exit 1
fi

echo "Converting desktop_icon.svg to Tauri icon formats..."
echo ""

# Check if we have the right tools
if ! command -v python3 &> /dev/null; then
    echo "ERROR: python3 not found"
    exit 1
fi

# Try to use cairosvg if available
if python3 -c "import cairosvg" 2>/dev/null; then
    echo "Using cairosvg for conversion..."
    python3 convert_svg_icons.py
else
    echo "cairosvg not available. Please install it:"
    echo "  pip3 install --user cairosvg Pillow"
    echo ""
    echo "Or install via Homebrew:"
    echo "  brew install cairo"
    echo "  pip3 install --user cairosvg Pillow"
    echo ""
    echo "Alternative: You can convert the SVG manually using:"
    echo "  - Online tool: https://cloudconvert.com/svg-to-png"
    echo "  - Or use Figma/Sketch/Adobe Illustrator"
    exit 1
fi

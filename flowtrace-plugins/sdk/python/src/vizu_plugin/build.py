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
Build Python plugin to WASM using componentize-py.

Usage:
    python -m flowtrace_plugin.build my_plugin.py -o my_plugin.wasm
    
    # Or using the CLI:
    flowtrace-plugin-build my_plugin.py -o my_plugin.wasm
"""

import subprocess
import sys
from pathlib import Path
from typing import Optional


def build_plugin(
    source: Path, 
    output: Path, 
    wit_path: Optional[Path] = None,
    world: str = "flowtrace-plugin"
) -> None:
    """
    Compile Python plugin to WASM component.
    
    Args:
        source: Path to the Python source file
        output: Path for the output WASM file
        wit_path: Path to WIT files (optional, uses bundled if not provided)
        world: WIT world name
    """
    if not source.exists():
        raise FileNotFoundError(f"Source file not found: {source}")
    
    # Find WIT files
    if wit_path is None:
        # Look for bundled WIT files
        package_dir = Path(__file__).parent
        wit_path = package_dir / "wit"
        
        if not wit_path.exists():
            # Try parent directory for development
            wit_path = package_dir.parent.parent / "wit"
    
    if not wit_path.exists():
        raise FileNotFoundError(
            f"WIT files not found at {wit_path}. "
            "Please provide --wit-path or ensure the SDK is properly installed."
        )
    
    # Build command
    cmd = [
        "componentize-py",
        "-d", str(wit_path),
        "-w", world,
        "componentize",
        str(source),
        "-o", str(output)
    ]
    
    print(f"Building {source} -> {output}")
    print(f"Command: {' '.join(cmd)}")
    
    try:
        result = subprocess.run(cmd, check=True, capture_output=True, text=True)
        if result.stdout:
            print(result.stdout)
        
        size_kb = output.stat().st_size / 1024
        print(f"✅ Built {output} ({size_kb:.1f} KB)")
        
    except subprocess.CalledProcessError as e:
        print(f"❌ Build failed: {e.stderr}", file=sys.stderr)
        sys.exit(1)
    except FileNotFoundError:
        print(
            "❌ componentize-py not found. Install it with:\n"
            "   pip install componentize-py",
            file=sys.stderr
        )
        sys.exit(1)


def main():
    """CLI entry point."""
    import argparse
    
    parser = argparse.ArgumentParser(
        description="Build Flowtrace Python plugin to WASM"
    )
    parser.add_argument(
        "source",
        type=Path,
        help="Path to Python source file"
    )
    parser.add_argument(
        "-o", "--output",
        type=Path,
        default=None,
        help="Output WASM file path (default: <source>.wasm)"
    )
    parser.add_argument(
        "--wit-path",
        type=Path,
        default=None,
        help="Path to WIT files"
    )
    parser.add_argument(
        "--world",
        type=str,
        default="flowtrace-plugin",
        help="WIT world name (default: flowtrace-plugin)"
    )
    
    args = parser.parse_args()
    
    output = args.output or args.source.with_suffix(".wasm")
    
    build_plugin(
        source=args.source,
        output=output,
        wit_path=args.wit_path,
        world=args.world
    )


if __name__ == "__main__":
    main()

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

"""Custom build script to include .pth file in wheel."""

import os
import shutil
import subprocess
import zipfile
import tempfile

def build_wheel_with_pth():
    """Build wheel and add .pth file."""
    
    print("🔨 Building wheel...")
    # Build the wheel normally
    result = subprocess.run(
        ["/opt/homebrew/bin/python3", "-m", "build", "--wheel"],
        capture_output=True,
        text=True
    )
    
    if result.returncode != 0:
        print(f"❌ Build failed: {result.stderr}")
        return False
    
    print("✅ Wheel built")
    
    # Find the wheel file
    wheel_file = None
    for file in os.listdir("dist"):
        if file.endswith(".whl"):
            wheel_file = os.path.join("dist", file)
            break
    
    if not wheel_file:
        print("❌ Wheel file not found")
        return False
    
    print(f"📦 Modifying wheel: {wheel_file}")
    
    # Create temporary directory
    with tempfile.TemporaryDirectory() as tmpdir:
        # Extract wheel
        with zipfile.ZipFile(wheel_file, 'r') as zf:
            zf.extractall(tmpdir)
        
        # Copy .pth file to root of wheel
        pth_source = "agentreplay-init.pth"
        pth_dest = os.path.join(tmpdir, "agentreplay-init.pth")
        shutil.copy2(pth_source, pth_dest)
        print(f"✅ Added .pth file")
        
        # Update RECORD file
        record_file = None
        for root, dirs, files in os.walk(tmpdir):
            for file in files:
                if file == "RECORD":
                    record_file = os.path.join(root, file)
                    break
        
        if record_file:
            with open(record_file, 'a') as f:
                f.write("agentreplay-init.pth,,\n")
            print(f"✅ Updated RECORD")
        
        # Recreate wheel
        os.remove(wheel_file)
        with zipfile.ZipFile(wheel_file, 'w', zipfile.ZIP_DEFLATED) as zf:
            for root, dirs, files in os.walk(tmpdir):
                for file in files:
                    file_path = os.path.join(root, file)
                    arcname = os.path.relpath(file_path, tmpdir)
                    zf.write(file_path, arcname)
        
        print(f"✅ Wheel updated: {wheel_file}")
    
    # Verify
    print("\n📋 Verifying wheel contents:")
    with zipfile.ZipFile(wheel_file, 'r') as zf:
        has_pth = False
        for name in zf.namelist():
            if name == "agentreplay-init.pth":
                has_pth = True
                print(f"   ✓ {name}")
        
        if not has_pth:
            print("   ❌ .pth file not found in wheel!")
            return False
    
    print(f"\n🎉 Wheel ready: {wheel_file}")
    return True

if __name__ == "__main__":
    os.chdir("/Users/sushanth/chronolake/sdks/python")
    
    # Clean previous builds
    if os.path.exists("dist"):
        shutil.rmtree("dist")
    if os.path.exists("build"):
        shutil.rmtree("build")
    
    success = build_wheel_with_pth()
    exit(0 if success else 1)

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

"""Fix the wheel to place .pth file in the correct location."""

import zipfile
import os
import tempfile
import shutil

wheel_path = "dist/agentreplay-0.1.0-py3-none-any.whl"

print(f"🔧 Fixing wheel: {wheel_path}")

# Create a temporary directory
with tempfile.TemporaryDirectory() as tmpdir:
    # Extract the wheel
    with zipfile.ZipFile(wheel_path, 'r') as zip_ref:
        zip_ref.extractall(tmpdir)
    
    # Find and move the .pth file
    pth_source = os.path.join(tmpdir, "agentreplay-0.1.0.data", "data", "agentreplay-init.pth")
    pth_dest = os.path.join(tmpdir, "agentreplay-init.pth")
    
    if os.path.exists(pth_source):
        shutil.move(pth_source, pth_dest)
        print(f"✅ Moved .pth file to root")
        
        # Remove empty data directory
        data_dir = os.path.join(tmpdir, "agentreplay-0.1.0.data")
        if os.path.exists(data_dir) and not os.listdir(os.path.join(data_dir, "data")):
            shutil.rmtree(data_dir)
            print(f"✅ Removed empty data directory")
    
    # Recreate the wheel
    os.remove(wheel_path)
    with zipfile.ZipFile(wheel_path, 'w', zipfile.ZIP_DEFLATED) as zip_ref:
        for root, dirs, files in os.walk(tmpdir):
            for file in files:
                file_path = os.path.join(root, file)
                arcname = os.path.relpath(file_path, tmpdir)
                zip_ref.write(file_path, arcname)
    
    print(f"✅ Fixed wheel created: {wheel_path}")

# Verify
print("\n📦 Wheel contents:")
with zipfile.ZipFile(wheel_path, 'r') as zip_ref:
    for name in sorted(zip_ref.namelist()):
        if '.pth' in name or 'data' in name:
            print(f"   {name}")

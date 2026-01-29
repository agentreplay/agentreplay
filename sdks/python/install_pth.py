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

"""Install agentreplay-init.pth to site-packages for auto-initialization."""

import os
import sys
import shutil

def install_pth():
    """Copy .pth file to site-packages."""
    # Find site-packages
    site_packages = None
    for path in sys.path:
        if 'site-packages' in path and os.path.isdir(path):
            site_packages = path
            break
    
    if not site_packages:
        print("❌ Could not find site-packages directory")
        return False
    
    # Source .pth file
    script_dir = os.path.dirname(os.path.abspath(__file__))
    source_pth = os.path.join(script_dir, 'agentreplay-init.pth')
    
    if not os.path.exists(source_pth):
        print(f"❌ Source .pth file not found: {source_pth}")
        return False
    
    # Destination
    dest_pth = os.path.join(site_packages, 'agentreplay-init.pth')
    
    try:
        shutil.copy2(source_pth, dest_pth)
        print(f"✅ Installed {dest_pth}")
        print(f"\n📋 Content:")
        with open(dest_pth) as f:
            print(f"   {f.read()}")
        print(f"\n🎉 Agentreplay will now auto-initialize when AGENTREPLAY_ENABLED=true")
        print(f"   Restart Python for changes to take effect.")
        return True
    except Exception as e:
        print(f"❌ Failed to install .pth file: {e}")
        return False

if __name__ == "__main__":
    install_pth()

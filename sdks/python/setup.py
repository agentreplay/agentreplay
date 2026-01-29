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

"""Setup script for agentreplay package with proper .pth file installation."""

from setuptools import setup
from setuptools.command.install import install
import os
import shutil


class PostInstallCommand(install):
    """Post-installation hook to install .pth file to site-packages."""
    
    def run(self):
        install.run(self)
        
        # Find site-packages directory
        for path in self.get_outputs():
            if 'site-packages' in path and '__init__.py' in path:
                site_packages = os.path.dirname(os.path.dirname(path))
                break
        else:
            print("Warning: Could not find site-packages directory")
            return
        
        # Copy .pth file to site-packages
        pth_source = os.path.join(os.path.dirname(__file__), 'agentreplay-init.pth')
        pth_dest = os.path.join(site_packages, 'agentreplay-init.pth')
        
        try:
            shutil.copy2(pth_source, pth_dest)
            print(f"✅ Installed {pth_dest}")
        except Exception as e:
            print(f"⚠️  Failed to install .pth file: {e}")


setup(
    cmdclass={
        'install': PostInstallCommand,
    }
)

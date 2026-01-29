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

import struct

# Read the 32x32 PNG
with open('32x32.png', 'rb') as f:
    png_data = f.read()

# ICO file format
ico_header = struct.pack('<HHH', 0, 1, 1)  # Reserved, Type (1=ICO), Count
ico_entry = struct.pack('<BBBBHHII', 
    32,  # Width
    32,  # Height
    0,   # Colors (0 for PNG)
    0,   # Reserved
    1,   # Color planes
    32,  # Bits per pixel
    len(png_data),  # Image size
    22   # Offset to image data (6 + 16 bytes)
)

with open('icon.ico', 'wb') as f:
    f.write(ico_header)
    f.write(ico_entry)
    f.write(png_data)

print('Created icon.ico successfully')

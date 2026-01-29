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

# Create minimal valid PNG files with RGBA

def create_png_rgba(width, height, filename):
    import struct
    import zlib
    
    # PNG signature
    png_signature = b'\x89PNG\r\n\x1a\n'
    
    # IHDR chunk (image header) - color type 6 = RGBA
    ihdr_data = struct.pack('>IIBBBBB', width, height, 8, 6, 0, 0, 0)
    ihdr = struct.pack('>I', 13) + b'IHDR' + ihdr_data
    ihdr += struct.pack('>I', zlib.crc32(b'IHDR' + ihdr_data) & 0xffffffff)
    
    # IDAT chunk (image data) - solid blue color with full opacity
    raw_data = bytearray()
    for y in range(height):
        raw_data.append(0)  # filter type
        for x in range(width):
            raw_data.extend([59, 130, 246, 255])  # RGBA: #3b82f6 (blue) + full alpha
    
    compressed_data = zlib.compress(bytes(raw_data), 9)
    idat = struct.pack('>I', len(compressed_data)) + b'IDAT' + compressed_data
    idat += struct.pack('>I', zlib.crc32(b'IDAT' + compressed_data) & 0xffffffff)
    
    # IEND chunk
    iend = struct.pack('>I', 0) + b'IEND'
    iend += struct.pack('>I', zlib.crc32(b'IEND') & 0xffffffff)
    
    # Write PNG file
    with open(filename, 'wb') as f:
        f.write(png_signature + ihdr + idat + iend)

# Create all required icons
create_png_rgba(32, 32, '32x32.png')
create_png_rgba(128, 128, '128x128.png')
create_png_rgba(128, 128, '128x128@2x.png')
create_png_rgba(256, 256, 'icon.png')

print('Created RGBA PNG icons successfully')

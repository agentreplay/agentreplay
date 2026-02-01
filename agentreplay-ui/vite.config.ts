// Copyright 2025 Sushanth (https://github.com/sushanthpy)
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import path from 'path';

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react()],

  // Clear the screen on each build
  clearScreen: false,

  // Tauri expects a fixed port, fail if that port is not available
  server: {
    port: 47173,
    strictPort: true,
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:47100',
        changeOrigin: true,
        secure: false,
        // Suppress connection errors during startup (backend may not be ready yet)
        configure: (proxy) => {
          proxy.on('error', () => {});
        },
      },
      '/ws': {
        target: 'ws://127.0.0.1:47100',
        ws: true,
      },
    },
  },

  // Environment variables with VITE_ prefix will be exposed to the frontend code
  envPrefix: ['VITE_', 'TAURI_'],

  build: {
    // Tauri supports es2021
    target: process.env.TAURI_PLATFORM == 'windows' ? 'chrome105' : 'safari13',
    // Don't minify for debug builds
    minify: !process.env.TAURI_DEBUG ? 'esbuild' : false,
    // Produce sourcemaps for debug builds
    sourcemap: !!process.env.TAURI_DEBUG,
    // Output directory
    outDir: 'dist',
  },

  resolve: {
    alias: {
      '@': path.resolve(__dirname, './'),
      '@/components': path.resolve(__dirname, './components'),
      '@/hooks': path.resolve(__dirname, './hooks'),
      '@/lib': path.resolve(__dirname, './lib'),
    },
  },
});

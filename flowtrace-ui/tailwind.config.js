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

/** @type {import('tailwindcss').Config} */
module.exports = {
	darkMode: ['class'],
	content: [
		'./pages/**/*.{js,ts,jsx,tsx,mdx}',
		'./components/**/*.{js,ts,jsx,tsx,mdx}',
		'./app/**/*.{js,ts,jsx,tsx,mdx}',
		'./src/**/*.{js,ts,jsx,tsx,mdx}',
	],
	theme: {
		extend: {
			fontFamily: {
				sans: [
					'-apple-system',
					'BlinkMacSystemFont',
					'"SF Pro Text"',
					'"Segoe UI"',
					'Roboto',
					'"Helvetica Neue"',
					'Arial',
					'sans-serif',
				],
				mono: ['"JetBrains Mono"', 'monospace'], // Keeping a good mono font fallback
			},
			colors: {
				// Mapping "Cash App" style names to semantic tokens
				primary: {
					DEFAULT: 'hsl(var(--primary) / <alpha-value>)', // #13bbaf (Teal)
					foreground: 'hsl(var(--primary-foreground) / <alpha-value>)',
				},
				accent: {
					DEFAULT: 'hsl(var(--accent) / <alpha-value>)', // #32353b
					foreground: 'hsl(var(--accent-foreground) / <alpha-value>)',
				},
				background: 'hsl(var(--background) / <alpha-value>)',
				foreground: 'hsl(var(--foreground) / <alpha-value>)',
				surface: {
					DEFAULT: 'hsl(var(--card) / <alpha-value>)',
					elevated: 'hsl(var(--secondary) / <alpha-value>)',
					hover: 'hsl(var(--surface-hover) / <alpha-value>)'
				},
				border: 'hsl(var(--border) / <alpha-value>)',

				// Custom Palette from sample.txt
				'block-teal': '#13bbaf',
				'block-orange': '#ff4f00',

				textPrimary: 'hsl(var(--foreground) / <alpha-value>)',
				textSecondary: 'hsl(var(--text-secondary) / <alpha-value>)',
				textTertiary: 'hsl(var(--text-tertiary) / <alpha-value>)',
				textMuted: 'hsl(var(--text-muted) / <alpha-value>)',

				success: {
					DEFAULT: '#91cb80', // green-200
					bg: 'rgba(145, 203, 128, 0.15)',
					foreground: '#003300'
				},
				warning: {
					DEFAULT: '#fbcd44', // yellow-200
					bg: 'rgba(251, 205, 68, 0.15)',
					foreground: '#443300'
				},
				error: {
					DEFAULT: '#f94b4b', // red-200
					bg: 'rgba(249, 75, 75, 0.15)',
					foreground: '#550000'
				},
				info: {
					DEFAULT: '#5c98f9', // blue-200
					bg: 'rgba(92, 152, 249, 0.15)',
					foreground: '#002255'
				},
				card: {
					DEFAULT: 'hsl(var(--card) / <alpha-value>)',
					foreground: 'hsl(var(--card-foreground) / <alpha-value>)'
				},
				popover: {
					DEFAULT: 'hsl(var(--popover) / <alpha-value>)',
					foreground: 'hsl(var(--popover-foreground) / <alpha-value>)'
				},
				secondary: {
					DEFAULT: 'hsl(var(--secondary) / <alpha-value>)',
					foreground: 'hsl(var(--secondary-foreground) / <alpha-value>)'
				},
				muted: {
					DEFAULT: 'hsl(var(--muted) / <alpha-value>)',
					foreground: 'hsl(var(--muted-foreground) / <alpha-value>)'
				},
				destructive: {
					DEFAULT: 'hsl(var(--destructive) / <alpha-value>)',
					foreground: 'hsl(var(--destructive-foreground) / <alpha-value>)'
				},
				input: 'hsl(var(--input) / <alpha-value>)',
				ring: 'hsl(var(--ring) / <alpha-value>)',
				chart: {
					'1': 'hsl(var(--chart-1) / <alpha-value>)',
					'2': 'hsl(var(--chart-2) / <alpha-value>)',
					'3': 'hsl(var(--chart-3) / <alpha-value>)',
					'4': 'hsl(var(--chart-4) / <alpha-value>)',
					'5': 'hsl(var(--chart-5) / <alpha-value>)'
				}
			},
			boxShadow: {
				sm: '0 1px 2px 0 rgba(0, 0, 0, 0.05)',
				md: '0 4px 6px -1px rgba(0, 0, 0, 0.1)',
				lg: '0 10px 15px -3px rgba(0, 0, 0, 0.1)',
				xl: '0 20px 25px -5px rgba(0, 0, 0, 0.1)',
				'custom-card': 'var(--shadow-default)',
			},
			animation: {
				'pulse-slow': 'pulse 3s cubic-bezier(0.4, 0, 0.6, 1) infinite',
				'fade-slide-up': 'fade-slide-up 0.5s cubic-bezier(0.16, 1, 0.3, 1) forwards',
				'appear': 'appear 0.5s ease-out forwards',
				'sidebar-item-in': 'sidebar-item-in 0.6s cubic-bezier(0.16, 1, 0.3, 1) forwards',
			},
			keyframes: {
				'fade-slide-up': {
					'0%': { opacity: '0', transform: 'translateY(20px)' },
					'100%': { opacity: '1', transform: 'translateY(0)' },
				},
				'appear': {
					'0%': { opacity: '0' },
					'100%': { opacity: '1' },
				},
				'sidebar-item-in': {
					'0%': { opacity: '0', transform: 'translateX(20px)' },
					'100%': { opacity: '1', transform: 'translateX(0)' },
				},
			},
			transitionTimingFunction: {
				'cash-ease': 'cubic-bezier(0.55, 0, 1, 0.45)', // ease-g2
			},
			borderRadius: {
				lg: 'var(--radius)',
				md: 'calc(var(--radius) - 2px)',
				sm: 'calc(var(--radius) - 4px)'
			}
		}
	},
	plugins: [require("tailwindcss-animate")],
};

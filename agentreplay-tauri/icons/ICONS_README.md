# Agent Replay Desktop Icons

## Required Icons

To complete the desktop application build, you'll need to provide icons in the following formats:

### Icon Sizes Required

- **32x32.png** - Small icon for Windows
- **128x128.png** - Medium icon for macOS and Linux
- **128x128@2x.png** - Retina display version
- **icon.icns** - macOS bundle icon (generated from PNG)
- **icon.ico** - Windows executable icon (generated from PNG)

### Generating Icons

You can use tools to generate all formats from a single high-resolution source image:

**Option 1: Online Tool**
- https://icon.kitchen/ - Free online icon generator

**Option 2: Tauri Icon Command**
```bash
npm run tauri icon path/to/source-image.png
```

This will automatically generate all required formats.

### Icon Design Guidelines

**Recommended:**
- Square icon with rounded corners
- Simple, recognizable design
- Works well at small sizes (16x16)
- Distinct silhouette
- Single primary color or gradient
- Avoid too much detail

**Color Scheme Suggestions:**
- Primary: #3B82F6 (blue) - represents data and technology
- Accent: #10B981 (green) - represents growth and monitoring
- Alternative: #8B5CF6 (purple) - represents AI and intelligence

### Agent Replay Icon Concept

The icon could represent:
- A clock/time element (chronos = time)
- A database/data structure
- An eye/monitoring symbol
- Flowing data or traces

**Suggested Design:**
A stylized clock icon with flowing data streams or a modern "CL" monogram with a time/flow element.

### Temporary Solution

For development, you can use any square PNG and run:
```bash
npm run tauri icon your-icon.png
```

This will generate all required formats automatically.

### Production Icons

For production releases, consider hiring a designer or using:
- Fiverr (https://www.fiverr.com/categories/graphics-design/creative-logo-design)
- 99designs (https://99designs.com/)
- Dribbble hiring (https://dribbble.com/hiring)

### Current Status

⚠️ **Action Required**: Icons need to be created before production build.

The application will build without custom icons (using Tauri defaults), but custom branding is recommended for professional releases.

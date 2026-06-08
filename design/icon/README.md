# Mooz — Icon Composer layers

Ready-to-import layers for Apple's **Icon Composer** (ships with Xcode 26) to
produce the layered "Liquid Glass" `.icon` for macOS 26 / iOS 26.

## Files
- `glyph.svg` / `glyph.png` — foreground: Phosphor `magnifying-glass-plus`
  (bold), centered on a 1024 canvas at ~57%, transparent background, graphite
  fill. SVG is vector (preferred); PNG is a 1024 fallback.
- `background.svg` / `background.png` — full-bleed light-grey → silver gradient.
  The system applies the rounded mask + glass, so it fills the whole canvas.

## Assemble in Icon Composer
1. Open **Icon Composer** (or Xcode ▸ Open Developer Tool ▸ Icon Composer).
2. New icon → name it `Mooz`.
3. Background: drag in `background.svg`, **or** use Icon Composer's built-in
   gradient picker (light grey → silver) and skip the file.
4. Foreground: drag `glyph.svg` on top as a new layer. Nudge to optical center
   if needed (the magnifier's weight sits slightly up-left of the handle).
5. Tune the glass: blur, specular highlight, shadow; preview the **Default /
   Dark / Tinted / Clear** appearances. For glass, a white or mid-tone glyph
   often reads better than graphite — recolor the layer in the inspector.
6. Export the `.icon` bundle.

## Wire into the project
- Drop the exported `Mooz.icon` into the Xcode project and set it as the
  app icon (Target ▸ App Icon, or replace `Assets.xcassets/AppIcon.appiconset`).
- In `project.yml`, point `ASSETCATALOG_COMPILER_APPICON_NAME` at the new icon
  name if it differs from `AppIcon`, then `xcodegen generate`.

The current flat `AppIcon.appiconset` keeps working everywhere; the `.icon` is
an upgrade for the macOS 26 glass look.

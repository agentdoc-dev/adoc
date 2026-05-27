# Data Image

The image embed below uses the `data:` scheme, which V4 Compatibility Mode
treats as unsafe: the renderer drops the `src` attribute (preserving the alt
text) and the compat validator emits one `compat.unsafe_image_src_dropped`
warning.

![transparent svg](data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIC8+)

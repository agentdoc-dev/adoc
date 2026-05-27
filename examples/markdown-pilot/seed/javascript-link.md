# JavaScript Link

The link below uses the `javascript:` scheme, which V4 Compatibility Mode
treats as unsafe: the rendered HTML drops the `href` attribute and the
compat validator emits one `compat.unsafe_link_dropped` warning.

Please do not [click here](javascript:alert(1)) — the rendered output keeps
the visible link text but no longer activates the JavaScript URL.

# Raw HTML

Block-level raw HTML is quarantined under V4 Compatibility Mode: the original
markup is preserved as escaped text inside a `<pre class="quarantined-html">`
wrapper, and the compat validator emits one warning per quarantined block.

<div>
  This block-level <div> renders as escaped source text in the HTML output
  and never reaches the browser as live markup.
</div>

<script>
  // Inline scripts are quarantined for the same reason: they would be
  // executable if the renderer passed them through unchanged.
  console.log("never executed");
</script>

Two `compat.raw_html_quarantined` warnings are expected for this file — one
per block above.

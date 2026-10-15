<script lang="ts" module>
import type { Colour } from "$lib/bindings";

/** The panel's tile sizes in CSS pixels: 32 in lists, 64 on finds. */
export type TileSize = 32 | 64;

const GLYPH_COLOURS: Record<Colour, string> = {
  black: "var(--dcss-black)",
  blue: "var(--dcss-blue)",
  green: "var(--dcss-green)",
  cyan: "var(--dcss-cyan)",
  red: "var(--dcss-red)",
  magenta: "var(--dcss-magenta)",
  brown: "var(--dcss-brown)",
  lightGrey: "var(--dcss-light-grey)",
  darkGrey: "var(--dcss-dark-grey)",
  lightBlue: "var(--dcss-light-blue)",
  lightGreen: "var(--dcss-light-green)",
  lightCyan: "var(--dcss-light-cyan)",
  lightRed: "var(--dcss-light-red)",
  lightMagenta: "var(--dcss-light-magenta)",
  yellow: "var(--dcss-yellow)",
  white: "var(--dcss-white)",
};

/**
 * The largest whole-number scale at which a tile `natural` device pixels wide fits a `size` px
 * slot, or 0 if it doesn't fit even at 1x. Scaling by whole numbers in device pixels keeps
 * every tile pixel square and crisp, whatever the display's pixel ratio.
 */
function wholeScale(natural: number, size: number, pixelRatio: number): number {
  return Math.floor((size * pixelRatio) / natural);
}
</script>

<script lang="ts">
import type { Appearance, Depiction } from "$lib/bindings";

let {
  depiction,
  appearance,
  size,
}: { depiction: Depiction; appearance: Appearance; size: TileSize } = $props();

const src = $derived(
  appearance === "tiles" && depiction.tile !== null ? `/tiles/${depiction.tile}.png` : null,
);

// The CSS size a loaded tile is drawn at, or `null` if it failed to load or can't be drawn at a
// whole-number scale, so its glyph shows instead. Kept per source, so a new tile measures afresh.
let fit = $state<{ src: string; px: number | null } | null>(null);
const fitted = $derived(fit !== null && fit.src === src ? fit : null);

function measured(event: Event, loaded: string) {
  const img = event.currentTarget as HTMLImageElement;
  const ratio = window.devicePixelRatio || 1;
  const scale = wholeScale(img.naturalWidth, size, ratio);
  const square = img.naturalWidth === img.naturalHeight;
  fit = { src: loaded, px: scale >= 1 && square ? (scale * img.naturalWidth) / ratio : null };
}
</script>

<span class="tile" style:width="{size}px" style:height="{size}px">
  {#if src && fitted?.px !== null}
    {#key src}
      <img
        {src}
        alt={depiction.glyph.text}
        style:width={fitted ? `${fitted.px}px` : undefined}
        class:measuring={!fitted}
        onload={(event) => measured(event, src)}
        onerror={() => {
          fit = { src, px: null };
        }}
      >
    {/key}
  {:else}
    <span
      class="glyph"
      style:color={GLYPH_COLOURS[depiction.glyph.colour]}
      style:font-size="{Math.round(size * 0.7)}px"
    >
      {depiction.glyph.text}
    </span>
  {/if}
</span>

<style>
.tile {
  display: inline-grid;
  place-items: center;
  flex: none;
}

img {
  image-rendering: pixelated;
}

/* Hidden until its natural size picks the scale it is drawn at. */
img.measuring {
  visibility: hidden;
}

.glyph {
  font-family: var(--mono);
  font-weight: 700;
  line-height: 1;
}
</style>

# Block texture sources

These source textures were generated with the built-in ImageGen tool, then
converted into neutral grayscale 64x64 runtime textures. The game embeds the
runtime copies and samples them with nearest-neighbor filtering.

## Riveted armor plate prompt

> Use case: stylized-concept  
> Asset type: square game texture for one voxel/block face  
> Primary request: an original industrial riveted armor-plate texture for a
> late-1990s software-rendered arena shooter  
> Subject: one square gunmetal plate with a shallow recessed center, chunky
> corner rivets, chipped edges, pitting, scratches, soot and sparse rust stains  
> Style/medium: hand-painted diffuse game texture, gritty old PC shooter
> aesthetic, bold chunky material definition that remains readable when reduced
> to 32x32 pixels  
> Composition/framing: perfectly front-facing orthographic square, texture fills
> the canvas edge to edge, symmetrical underlying plate structure with irregular
> organic wear  
> Lighting/mood: mostly flat baked diffuse illumination, restrained top-edge
> highlight, no dramatic cast shadows  
> Color palette: neutral grayscale gunmetal with small muted brown rust accents;
> designed to be multiplied by strong gameplay colors  
> Constraints: single material texture only; no scene, no perspective, no text,
> no letters, no numbers, no symbols, no logos, no watermark, no border outside
> the texture, no photorealistic depth of field  
> Avoid: tiny high-frequency noise, glossy reflections, colorful paint, obvious
> focal illustration

## Vented panel prompt

> Use case: stylized-concept  
> Asset type: square game texture for one voxel/block face  
> Primary request: an original battered stamped-steel vent panel texture for a
> late-1990s software-rendered industrial arena shooter  
> Subject: one square steel panel with a bold shallow X-shaped stamped brace,
> four broad ventilation slots in the center, two large fasteners, worn corners,
> chipped metal, scratches, soot and sparse rust  
> Style/medium: hand-painted diffuse game texture, gritty old PC shooter
> aesthetic, simplified chunky forms and high-contrast wear that remain readable
> when reduced to 32x32 pixels  
> Composition/framing: perfectly front-facing orthographic square, texture fills
> the canvas edge to edge, strong centered panel structure, irregular organic
> damage  
> Lighting/mood: mostly flat baked diffuse illumination, restrained upper-edge
> highlight, no dramatic cast shadows  
> Color palette: medium-light neutral grayscale steel with tiny muted brown rust
> accents; designed to be multiplied by saturated gameplay colors  
> Constraints: single material texture only; no scene, no perspective, no text,
> no letters, no numbers, no symbols, no logos, no watermark, no border outside
> the texture, no photorealistic depth of field  
> Avoid: tiny high-frequency noise, glossy reflections, colorful paint, wires,
> pipes, gauges, obvious focal illustration

## Recessed bezel gunmetal prompt

> Use case: stylized-concept  
> Asset type: seamless tileable game texture for the continuous metal rails
> around a Tetris playfield  
> Primary request: an original worn gunmetal surface texture for a late-1990s
> software-rendered industrial arena shooter  
> Subject: continuous flat steel with broad mottled grime, chipped finish,
> sparse rust freckles, shallow scratches, pitting, soot and subtle worn
> highlights  
> Style/medium: hand-painted diffuse game texture, gritty old PC shooter
> aesthetic, with chunky low-frequency detail that survives reduction  
> Composition/framing: perfectly front-facing orthographic square, edge-to-edge
> and seamless, with no perspective  
> Lighting/mood: mostly flat baked diffuse illumination, restrained contrast,
> no cast shadows  
> Color palette: neutral grayscale gunmetal with extremely sparse muted brown
> oxidation; designed to be tinted dark warm gray in game  
> Constraints: homogeneous material only; no panel border, frame, rivets,
> bolts, vents, seams, grid, focal mark, text, symbols, logos or watermark  
> Avoid: obvious square composition, high-frequency static noise, glossy
> reflections, colorful paint, large structural shapes, dramatic lighting

## Runtime conversion

Both runtime assets were made with this point-sampled pipeline:

```sh
magick SOURCE.png -colorspace Gray -contrast-stretch 1%x1% \
  +level 42%,100% -filter point -resize 64x64! -colorspace sRGB OUTPUT.png
```

The bezel material uses the same grayscale and point-sampling treatment and is
reduced to one seamless 64x64 tile. The fascia repeats it at exactly one tile
per world unit, preserving square texels at the playfield's 17-pixel cell pitch
instead of stretching one image over the full height. The underlying fascia
and throat remain continuous geometry, and every runtime texture uses nearest
filtering.

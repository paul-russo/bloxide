# Texture sources

The game no longer ships texture files. Every material (riveted armour plate,
vented panel, gunmetal, the stone wall) is generated at startup by
`src/textures.rs`, authored at the exact texel density it is displayed at:
one texel per framebuffer pixel on a 16-pixel block face, so bevels, rivets and
vent slots land on whole pixels instead of shimmering through a point sampler.

The images in this directory are the original ImageGen concept art that the
procedural materials were designed from. They are kept for reference and are
not loaded by the game.

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

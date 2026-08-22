/* Noctis IV Plus — WASM demo.
 *
 * Drives the nivgen.wasm exports (raw C ABI, no wasm-bindgen) and renders
 * every stage of the deterministic universe generator onto canvases.
 * See ../tests/nivgen/src/wasm.rs for the export list.
 */
"use strict";

const CLASS_NAMES = [
  "yellow", "red", "white-blue", "blue", "orange", "white", "green",
  "blue giant", "red dwarf", "violet", "white dwarf", "pulsar",
];

const CLASS_PLANETS = [12, 18, 8, 15, 20, 3, 0, 1, 7, 20, 2, 5];
const CLASS_RAY = [5000, 15000, 300, 20000, 15000, 1000, 3000, 2000, 4000, 1500, 30000, 250];
const CLASS_RGB = [
  [63, 58, 40], [30, 50, 63], [63, 63, 63], [63, 30, 20], [63, 55, 32], [32, 16, 10],
  [32, 28, 24], [10, 20, 63], [63, 32, 16], [48, 32, 63], [40, 10, 10], [0, 63, 63],
];

const PLANET_DESCRIPTION = [
  "internally hot, unstable surface, no atmosphere",
  "small, solid, dusty, craterized, no atmosphere",
  "thick atmosphere, fully covered by clouds",
  "felisian, breathable atmosphere, suitable for life",
  "rocky, creased, no atmosphere",
  "small, solid, thin atmosphere",
  "large, dense clouds",
  "small, solid, icy surface, no atmosphere",
  "native quartz surface, oxygen atmosphere",
  "very large substellar object",
  "companion star — not a planet",
];

const OBJ_CLASS_COLOR = [
  [154, 160, 168],  // 0 rocks
  [95, 163, 79],    // 1 vegetation
  [47, 107, 30],    // 2 trees
  [200, 178, 106],  // 3 crystal
];
const RUIN_COLOR = [217, 160, 102];

const SURFACE_W = 360, SURFACE_H = 180;
const SECTOR_N = 200;
const SURFTEX_N = 256;

/* ------------------------------------------------------------------ */
/* WASM bootstrap                                                     */
/* ------------------------------------------------------------------ */

let wasm = null;
let memory = null;
let ready = false;

function readBytes(ptr, len) {
  return new Uint8Array(memory.buffer, ptr, len).slice();
}

function status(msg, err) {
  const el = document.getElementById("status");
  el.textContent = msg;
  el.classList.remove("hidden", "err");
  if (err) el.classList.add("err");
  clearTimeout(status._t);
  status._t = setTimeout(() => el.classList.add("hidden"), err ? 5000 : 2200);
}

async function loadWasm() {
  const resp = await fetch("nivgen.wasm");
  if (!resp.ok) throw new Error("failed to fetch nivgen.wasm (" + resp.status + ")");
  const bytes = await resp.arrayBuffer();
  const { instance } = await WebAssembly.instantiate(bytes, {});
  wasm = instance.exports;
  memory = wasm.memory;
  wasm.niv_init();
  ready = true;
}

/* ------------------------------------------------------------------ */
/* Palette helpers                                                    */
/* ------------------------------------------------------------------ */

function paletteToRGBA(palBytes, base, out) {
  // 6-bit palette entry -> 8-bit rgba
  for (let i = 0; i < 64; i++) {
    const r = Math.min(255, palBytes[base + i * 3] * 4.05);
    const g = Math.min(255, palBytes[base + i * 3 + 1] * 4.05);
    const b = Math.min(255, palBytes[base + i * 3 + 2] * 4.05);
    out[i * 4] = r; out[i * 4 + 1] = g; out[i * 4 + 2] = b; out[i * 4 + 3] = 255;
  }
}

/** Render a width*height byte map as indexed-colour pixels (shared LUT). */
function renderIndexed(canvas, bytes, width, height, lut) {
  const ctx = canvas.getContext("2d");
  const img = ctx.createImageData(width, height);
  const d = img.data;
  for (let i = 0; i < bytes.length; i++) {
    const v = bytes[i] & 63;
    const o = i * 4;
    d[o] = lut[v * 4]; d[o + 1] = lut[v * 4 + 1]; d[o + 2] = lut[v * 4 + 2]; d[o + 3] = 255;
  }
  ctx.putImageData(img, 0, 0);
}

function renderGray(canvas, bytes, width, height, maxVal) {
  const ctx = canvas.getContext("2d");
  const img = ctx.createImageData(width, height);
  const d = img.data;
  const scale = 255 / maxVal;
  for (let i = 0; i < bytes.length; i++) {
    const v = Math.min(maxVal, bytes[i]) * scale;
    const o = i * 4;
    d[o] = v; d[o + 1] = v; d[o + 2] = v; d[o + 3] = 255;
  }
  ctx.putImageData(img, 0, 0);
}

function hex(u32) {
  return "0x" + (u32 >>> 0).toString(16).toUpperCase().padStart(8, "0");
}

/* ------------------------------------------------------------------ */
/* Star system                                                        */
/* ------------------------------------------------------------------ */

let curBody = -1;

function clearBodies() {
  const tb = document.querySelector("#bodies tbody");
  tb.innerHTML = "";
}

function selectBody(n, scroll) {
  curBody = n;
  document.querySelectorAll("#bodies tbody tr").forEach((tr) => {
    tr.classList.toggle("selected", +tr.dataset.idx === n);
  });
  if (scroll) document.getElementById("planet").scrollIntoView({ behavior: "smooth", block: "start" });
}

function typeName(t) {
  return t + " — " + PLANET_DESCRIPTION[t] || "";
}

function renderSystem() {
  const nob = wasm.niv_nob();
  const nop = wasm.niv_nop();
  const cls = wasm.niv_star_class();
  const ray = wasm.niv_star_ray();
  const spin = wasm.niv_star_spin();
  const identity = wasm.niv_star_identity();
  const [r, g, b] = CLASS_RGB[cls] || [255, 255, 255];

  const info = document.getElementById("starinfo");
  info.innerHTML =
    starCard("Star class", `<span class="swatch" style="background:rgb(${r * 4.05},${g * 4.05},${b * 4.05});color:rgb(${r * 4.05},${g * 4.05},${b * 4.05})"></span>${cls} ${CLASS_NAMES[cls]}`) +
    starCard("Ray", ray.toFixed(3)) +
    starCard("Spin", spin) +
    starCard("Identity", identity.toPrecision(6)) +
    starCard("Planets", nop) +
    starCard("Bodies", nob) +
    starCard("RGB", `${r},${g},${b}`);

  // orbit map
  const oc = document.getElementById("orbit");
  const ctx = oc.getContext("2d");
  ctx.clearRect(0, 0, oc.width, oc.height);

  let maxOrb = 1e-6;
  for (let i = 0; i < nob; i++) {
    if (wasm.niv_body_owner(i) === -1) maxOrb = Math.max(maxOrb, wasm.niv_body_orb_ray(i));
  }
  const scale = 190 / Math.sqrt(maxOrb);
  const cx = oc.width / 2, cy = oc.height / 2;

  // star
  const starRad = Math.max(5, Math.min(26, Math.sqrt(ray) * 5));
  const grd = ctx.createRadialGradient(cx, cy, 1, cx, cy, starRad * 2.6);
  grd.addColorStop(0, `rgba(${r * 4.05},${g * 4.05},${b * 4.05},0.9)`);
  grd.addColorStop(1, "rgba(0,0,0,0)");
  ctx.fillStyle = grd;
  ctx.beginPath(); ctx.arc(cx, cy, starRad * 2.6, 0, Math.PI * 2); ctx.fill();

  const planetPos = new Map();
  for (let i = 0; i < nob; i++) {
    const isMoon = wasm.niv_body_owner(i) !== -1;
    if (isMoon) continue;
    const orb = wasm.niv_body_orb_ray(i);
    const orbPx = Math.sqrt(orb) * scale;
    const type = wasm.niv_body_type(i);
    const ring = wasm.niv_body_ring(i);
    const col = typeColor(type, cls, i);
    // orbit path
    ctx.strokeStyle = "rgba(120,150,200,0.18)";
    ctx.lineWidth = 1;
    ctx.beginPath(); ctx.arc(cx, cy, orbPx, 0, Math.PI * 2); ctx.stroke();
    // planet
    const pr = Math.max(2, Math.min(11, Math.sqrt(wasm.niv_body_ray(i)) * 34));
    ctx.fillStyle = col;
    ctx.beginPath(); ctx.arc(cx, cy, pr, 0, Math.PI * 2); ctx.fill();
    // ring
    if (ring > 0) {
      ctx.strokeStyle = "rgba(200,210,255,0.75)";
      ctx.lineWidth = 1.5;
      ctx.beginPath(); ctx.ellipse(cx, cy, pr * 2.2, pr * 0.9, -0.4, 0, Math.PI * 2); ctx.stroke();
    }
    planetPos.set(i, { x: cx, y: cy, pr });
  }

  // moons (orbits around their owner drawn small)
  for (let i = 0; i < nob; i++) {
    const owner = wasm.niv_body_owner(i);
    if (owner === -1) continue;
    const op = planetPos.get(owner);
    if (!op) continue;
    const orb = wasm.niv_body_orb_ray(i);
    const orbPx = Math.max(8, Math.min(op.pr * 2.4, Math.sqrt(orb) * scale * 2));
    const a = (i * 137.5) * Math.PI / 180;
    const x = op.x + Math.cos(a) * orbPx;
    const y = op.y + Math.sin(a) * orbPx;
    ctx.fillStyle = "rgba(200,205,215,0.9)";
    ctx.beginPath(); ctx.arc(x, y, 1.8, 0, Math.PI * 2); ctx.fill();
  }

  // body table
  const tb = document.querySelector("#bodies tbody");
  tb.innerHTML = "";
  for (let i = 0; i < nob; i++) {
    const owner = wasm.niv_body_owner(i);
    const type = wasm.niv_body_type(i);
    const tr = document.createElement("tr");
    tr.dataset.idx = i;
    tr.innerHTML =
      `<td>${i}</td>` +
      `<td title="${typeName(type)}">${type}</td>` +
      `<td>${owner === -1 ? "★" : owner}</td>` +
      `<td>${wasm.niv_body_ray(i).toFixed(4)}</td>` +
      `<td>${owner === -1 ? wasm.niv_body_orb_ray(i).toFixed(3) : "—"}</td>` +
      `<td>${(wasm.niv_body_tilt(i) * 57.3).toFixed(1)}°</td>` +
      `<td>${wasm.niv_body_orb_ecc(i).toFixed(3)}</td>` +
      `<td>${wasm.niv_body_ring(i) > 0 ? "●" : "—"}</td>` +
      `<td>${wasm.niv_body_rtperiod(i) || "—"}</td>` +
      `<td><button class="surfbtn">Surface</button> <button class="landbtn">Land</button></td>`;
    tr.querySelector(".surfbtn").addEventListener("click", () => { selectBody(i, true); renderSurface(); });
    tr.querySelector(".landbtn").addEventListener("click", () => { selectBody(i, true); doLand(); });
    tb.appendChild(tr);
  }
  document.getElementById("system").classList.remove("hidden");
  document.getElementById("planet").classList.remove("hidden");
  document.getElementById("sector").classList.remove("hidden");
}

function starCard(k, v) {
  return `<div class="card"><div class="k">${k}</div><div class="v">${v}</div></div>`;
}

function typeColor(type, starClass, i) {
  const hues = [
    [210, 90, 50], [150, 130, 110], [240, 235, 230], [120, 190, 120], [200, 110, 80],
    [170, 150, 120], [225, 215, 180], [230, 240, 250], [160, 220, 210], [190, 160, 130],
    [255, 200, 90],
  ];
  const c = hues[type] || [180, 180, 180];
  const jitter = ((i * 37) % 21) - 10;
  return `rgb(${c[0] + jitter},${c[1] + jitter},${c[2] + jitter})`;
}

/* ------------------------------------------------------------------ */
/* Planet surface                                                     */
/* ------------------------------------------------------------------ */

function renderSurface() {
  const g = wasm;
  if (curBody < 0 || curBody >= g.niv_nob()) {
    status("select a body first", true);
    return;
  }
  g.niv_planet_surface(curBody);
  const owner = g.niv_body_owner(curBody);
  const type = g.niv_body_type(curBody);
  const palBase = g.niv_pal_base();

  const surface = readBytes(g.niv_surface_ptr(), SURFACE_W * SURFACE_H);
  const atmo = readBytes(g.niv_atmo_ptr(), 32400);
  const pal = readBytes(g.niv_pal_ptr(), 768);

  // build LUTs
  const palLut = new Uint8ClampedArray(256);
  paletteToRGBA(pal, palBase, palLut);

  document.getElementById("planettitle").textContent =
    `body ${curBody} · type ${type} ${owner === -1 ? "planet" : "moon of " + owner}`;

  // surface with optional atmosphere overlay
  const surfCtx = document.getElementById("surface").getContext("2d");
  const showAtmo = document.getElementById("atmo").checked;
  const img = surfCtx.createImageData(SURFACE_W, SURFACE_H);
  const d = img.data;
  for (let i = 0; i < SURFACE_W * SURFACE_H; i++) {
    let v = surface[i];
    if (showAtmo) v = Math.min(62, v + atmo[i]); // C merge: surface + atmo, cap 0x3E
    const o = i * 4;
    const lut = palLut[(v & 63) * 4];
    d[o] = lut; d[o + 1] = palLut[(v & 63) * 4 + 1]; d[o + 2] = palLut[(v & 63) * 4 + 2]; d[o + 3] = 255;
  }
  surfCtx.putImageData(img, 0, 0);

  // palette strip
  const palCtx = document.getElementById("palette").getContext("2d");
  const pimg = palCtx.createImageData(256, 32);
  for (let y = 0; y < 32; y++) {
    for (let x = 0; x < 256; x++) {
      const idx = Math.floor(x / 4) * 4;
      const o = (y * 256 + x) * 4;
      pimg.data[o] = palLut[idx];
      pimg.data[o + 1] = palLut[idx + 1];
      pimg.data[o + 2] = palLut[idx + 2];
      pimg.data[o + 3] = 255;
    }
  }
  palCtx.putImageData(pimg, 0, 0);

  document.getElementById("surfhash").textContent =
    `surf ${hex(g.niv_surface_fnv())} · atmo ${hex(g.niv_atmo_fnv())} · pal ${hex(g.niv_pal_fnv())}`;
}

/* ------------------------------------------------------------------ */
/* Landing sector                                                     */
/* ------------------------------------------------------------------ */

function renderObjChart(bytes) {
  const canvas = document.getElementById("objchart");
  const ctx = canvas.getContext("2d");
  const img = ctx.createImageData(SECTOR_N, SECTOR_N);
  const d = img.data;
  for (let i = 0; i < SECTOR_N * SECTOR_N; i++) {
    const b = bytes[i];
    const o = i * 4;
    if (b === 0x40) {
      // ruins (AF1 marker written raw)
      d[o] = RUIN_COLOR[0]; d[o + 1] = RUIN_COLOR[1]; d[o + 2] = RUIN_COLOR[2]; d[o + 3] = 255;
      continue;
    }
    const nr = b & 3;
    const c0 = (b >> 2) & 3;
    const c1 = (b >> 4) & 3;
    const c2 = (b >> 6) & 3;
    const cls = c2 !== 0 ? c2 : c1 !== 0 ? c1 : c0;
    const col = OBJ_CLASS_COLOR[cls] || [0, 0, 0];
    if (nr > 0) {
      const a = 0.6 + 0.13 * nr;
      d[o] = col[0]; d[o + 1] = col[1]; d[o + 2] = col[2]; d[o + 3] = a * 255;
    } else {
      const t = 26 + 6 * cls;
      d[o] = t; d[o + 1] = t + 8; d[o + 2] = t + 4; d[o + 3] = 90;
    }
  }
  ctx.putImageData(img, 0, 0);
}

function doLand() {
  const g = wasm;
  if (curBody < 0 || curBody >= g.niv_nob()) {
    status("select a body first", true);
    return;
  }
  const lon = parseInt(document.getElementById("lon").value, 10) || 0;
  const lat = parseInt(document.getElementById("lat").value, 10) || 60;
  let albedo = parseInt(document.getElementById("albedo").value, 10);
  if (isNaN(albedo)) albedo = -1;

  document.getElementById("sectortitle").textContent =
    `body ${curBody} · lon ${lon}° · lat ${lat}°`;

  if (!g.niv_sector(curBody, lon, lat, albedo, -1)) {
    status("body out of range", true);
    return;
  }

  const hm = readBytes(g.niv_heightmap_ptr(), SECTOR_N * SECTOR_N);
  const oc = readBytes(g.niv_objectchart_ptr(), SECTOR_N * SECTOR_N);
  const tex = readBytes(g.niv_surftex_ptr(), SURFTEX_N * SURFTEX_N);

  renderGray(document.getElementById("heightmap"), hm, SECTOR_N, SECTOR_N, 255);
  renderObjChart(oc);

  // sky + environment
  g.niv_sky();
  const sky = readBytes(g.niv_sky_ptr(), SURFACE_W * SURFACE_H);
  const pal = readBytes(g.niv_pal_ptr(), 768);

  // surftex uses the ground gradient (palette 0..63) written by create_sky
  const groundLut = new Uint8ClampedArray(256);
  paletteToRGBA(pal, 0, groundLut);
  renderIndexed(document.getElementById("surftex"), tex, SURFTEX_N, SURFTEX_N, groundLut);

  // sky uses palette 64..128 (sky gradient)
  const skyLut = new Uint8ClampedArray(256);
  paletteToRGBA(pal, 64, skyLut);
  renderIndexed(document.getElementById("sky"), sky, SURFACE_W, SURFACE_H, skyLut);

  // environment readout
  const sctype = g.niv_sctype();
  const scNames = ["?", "OCEAN", "PLAINS", "DESERT", "ICY"];
  const env = document.getElementById("env");
  env.innerHTML = [
    chip("scenario", scNames[sctype] || "?"),
    chip("albedo", g.niv_albedo()),
    chip("latitude", g.niv_latitude().toFixed(1)),
    chip("nightzone", g.niv_nightzone()),
    chip("temperature", g.niv_env_temp().toFixed(1) + " K"),
    chip("pressure", g.niv_env_pressure().toFixed(3) + " atm"),
    chip("surface seed", g.niv_global_surface_seed().toFixed(0)),
    chip("t-scale", g.niv_t_scale().toFixed(0)),
    chip("rockscale", g.niv_rockscaling().toFixed(1)),
    chip("rockpeak", g.niv_rockpeaking().toFixed(1)),
    chip("rockdens", g.niv_rockdensity()),
    chip("treescale", g.niv_treescaling().toFixed(0)),
    chip("quartz", g.niv_quartz()),
    chip("groundflares", g.niv_groundflares()),
  ].join("");

  document.getElementById("sectorhash").textContent =
    `hm ${hex(g.niv_heightmap_fnv())} · obj ${hex(g.niv_objectchart_fnv())} · ` +
    `stex ${hex(g.niv_surftex_fnv())} · sky ${hex(g.niv_sky_fnv())}`;

  status(`landed on body ${curBody} at ${lon}°,${lat}°`);
}

function chip(k, v) {
  return `<span class="chip"><span class="k">${k}:</span> <b>${v}</b></span>`;
}

/* ------------------------------------------------------------------ */
/* RNG lab                                                            */
/* ------------------------------------------------------------------ */

function renderRng() {
  const seed = parseInt(document.getElementById("rngseed").value, 10) || 0;
  wasm.niv_rng_srand(seed);
  const seq = [];
  for (let i = 0; i < 8; i++) seq.push(wasm.niv_rng_rand());
  document.getElementById("rngseq").textContent = seq.join(", ");
  const ok = seed === 1 &&
    seq.join(",") === "346,130,10982,1090,11656,7117,17595,6415";
  document.getElementById("rngcheck").textContent = ok
    ? "✓ matches canonical vector"
    : "seed 1 should give 346, 130, 10982, 1090, …";
  document.getElementById("rngcheck").className = "check " + (ok ? "ok" : "bad");

  const fseed = parseInt(document.getElementById("frngseed").value, 10) || 0;
  wasm.niv_frng_srand(fseed);
  const fseq = [];
  for (let i = 0; i < 8; i++) fseq.push(wasm.niv_frng_rand(0x7fff));
  document.getElementById("frngseq").textContent = fseq.join(", ");
  const fok = fseed === 12345 &&
    fseq.join(",") === "11673,14109,2627,8846,6091,22323,4112,10725";
  document.getElementById("frngcheck").textContent = fok
    ? "✓ matches canonical vector"
    : "seed 12345 should give 11673, 14109, 2627, 8846, …";
  document.getElementById("frngcheck").className = "check " + (fok ? "ok" : "bad");

  const dseed = parseInt(document.getElementById("frandseed").value, 10) || 0;
  wasm.niv_rng_srand(dseed);
  const f = wasm.niv_rng_flandom();
  document.getElementById("frandseq").textContent =
    "flandom(1) = " + f.toFixed(8) + (dseed === 1 && Math.abs(f - 0.00027466) < 1e-7 ? "  ✓" : "");
}

/* ------------------------------------------------------------------ */
/* Main flow                                                          */
/* ------------------------------------------------------------------ */

function generateSystem(x, y, z) {
  status("generating universe…");
  wasm.niv_generate(x, y, z);
  clearBodies();
  renderSystem();
  if (wasm.niv_nob() === 0) {
    status("degenerate system (0 bodies) — check coordinates", true);
    return;
  }
  curBody = 0;
  selectBody(0, false);
  renderSurface();
  doLand();
  document.getElementById("system").scrollIntoView({ behavior: "smooth", block: "start" });
  status("universe generated — every pixel derived from x,y,z");
}

function randomCoords() {
  const rnd = () => (Math.random() * 2 - 1) * 2.0e8;
  document.getElementById("x").value = rnd().toFixed(0);
  document.getElementById("y").value = rnd().toFixed(0);
  document.getElementById("z").value = rnd().toFixed(0);
}

function wire() {
  document.getElementById("gen").addEventListener("click", () => {
    generateSystem(
      parseFloat(document.getElementById("x").value),
      parseFloat(document.getElementById("y").value),
      parseFloat(document.getElementById("z").value));
  });

  document.getElementById("rand").addEventListener("click", () => {
    randomCoords();
    generateSystem(
      parseFloat(document.getElementById("x").value),
      parseFloat(document.getElementById("y").value),
      parseFloat(document.getElementById("z").value));
  });

  document.querySelectorAll(".presets button").forEach((b) => {
    b.addEventListener("click", () => {
      document.getElementById("x").value = b.dataset.x;
      document.getElementById("y").value = b.dataset.y;
      document.getElementById("z").value = b.dataset.z;
      generateSystem(+b.dataset.x, +b.dataset.y, +b.dataset.z);
    });
  });

  document.getElementById("rendersurf").addEventListener("click", renderSurface);
  document.getElementById("atmo").addEventListener("change", () => {
    if (curBody >= 0) renderSurface();
  });
  document.getElementById("land").addEventListener("click", doLand);

  document.getElementById("rnggo").addEventListener("click", renderRng);
  document.getElementById("frnggo").addEventListener("click", renderRng);
  document.getElementById("frandgo").addEventListener("click", renderRng);
}

(async function main() {
  try {
    await loadWasm();
    wire();
    renderRng();
    generateSystem(
      parseFloat(document.getElementById("x").value),
      parseFloat(document.getElementById("y").value),
      parseFloat(document.getElementById("z").value));
  } catch (e) {
    status("WASM load failed: " + e.message, true);
    console.error(e);
  }
})();

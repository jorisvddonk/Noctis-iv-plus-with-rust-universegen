#!/usr/bin/env -S deno run -A
/**
 * Validates an engine against the C oracle without injecting the farmalloc gap.
 *
 * Both ports derive the 16-byte header between p_surfacemap and objectschart
 * themselves, so nothing here passes -gap: this is exactly how the pipeline now
 * invokes them. Hashes are compared against the `orig_*` columns, which are the
 * C build's output.
 *
 * Usage:
 *   SHEETBOT_AUTH_APIKEY=... deno run -A tests/nivgen/tools/validate_gap.ts \
 *       --engine rust --stars all --out /tmp/rust.json
 *       --engine lino --stars all --out /tmp/lino.json
 *       --engine lino --stars 10 --fields all      # include surf_texture/sky_texture
 *       --star "MIDORI" --jobs 8                   # one star, parallel
 *
 * Resumable (checkpoint file), parallel, exits non-zero on any mismatch.
 */
const BASE = Deno.env.get("SHEETBOT_BASEURL") ?? "https://litterbox.moos.es";
const KEY = Deno.env.get("SHEETBOT_AUTH_APIKEY");
if (!KEY) {
  console.error("SHEETBOT_AUTH_APIKEY is required (see ~/.config/sheetbot/process-compose.yml)");
  Deno.exit(2);
}
const REPO = new URL("../../../", import.meta.url).pathname.replace(/\/$/, "");
const NIVGEN = `${REPO}/tests/nivgen/target/release/nivgen`;
const NIVLIN_DIR = Deno.env.get("NIVLIN_DIR") ?? `${REPO}/../noctis-lino`;
const NIVLIN = `${NIVLIN_DIR}/tools/nivlin`;

function arg(name: string, dflt?: string): string | undefined {
  const i = Deno.args.indexOf(`--${name}`);
  return i >= 0 ? Deno.args[i + 1] : dflt;
}

const ENGINE = arg("engine", "rust")!;
if (ENGINE !== "rust" && ENGINE !== "lino") throw new Error("--engine must be rust or lino");
const FIELDS_ARG = arg("fields", "sector")!; // sector | all

const login = await fetch(`${BASE}/login`, {
  method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ apiKey: KEY }),
});
if (!login.ok) throw new Error(`login failed: ${login.status}`);
const token = (await login.json()).token as string;
const H = { authorization: `Bearer ${token}` };

async function sheet(id: string): Promise<{ columns: { name: string }[]; rows: unknown[][] }> {
  for (let attempt = 0; attempt < 5; attempt++) {
    const r = await fetch(`${BASE}/sheets/${id}`, { headers: H });
    const t = await r.text();
    if (r.ok && t.trim().startsWith("{")) return JSON.parse(t);
    await new Promise((res) => setTimeout(res, 1500));
  }
  throw new Error(`sheet ${id} unavailable`);
}
type Row = Record<string, unknown>;
function toObjs(s: { columns: { name: string }[]; rows: unknown[][] }): Row[] {
  return s.rows.map((r) => {
    const o: Row = {};
    s.columns.forEach((c, i) => { o[c.name] = r[i]; });
    return o;
  });
}

function parseSector(text: string) {
  return {
    hm: text.match(/^heightmap\s+len=\d+\s+fnv=(\w+)/m)?.[1],
    oc: text.match(/^objectchart\s+len=\d+\s+fnv=(\w+)/m)?.[1],
  };
}
function parseTex(text: string) {
  return {
    stex: text.match(/^surf_texture\s+len=\d+\s+fnv=(\w+)/m)?.[1],
    sky: text.match(/^sky_texture\s+len=\d+\s+fnv=(\w+)/m)?.[1],
  };
}

/** Runs the engine with NO -gap, exactly as the pipeline does. */
function run(cmd: "sector" | "surftex", x: number, y: number, z: number, body: number, lon: number, lat: number): string {
  const common = ["-x", String(x), "-y", String(y), "-z", String(z), "-p", String(body),
    "-lon", String(lon), "-lat", String(lat)];
  if (ENGINE === "rust") {
    const p = new Deno.Command(NIVGEN, { args: [cmd, ...common], cwd: REPO, stdout: "piped", stderr: "null" }).outputSync();
    return new TextDecoder().decode(p.stdout);
  }
  const out = `/tmp/nivlin_${crypto.randomUUID()}.txt`;
  new Deno.Command(NIVLIN, { args: [cmd, ...common, "-o", out], cwd: REPO, stdout: "null", stderr: "null" }).outputSync();
  try { return Deno.readTextFileSync(out); } finally { try { Deno.removeSync(out); } catch { /* ignore */ } }
}

const stars = toObjs(await sheet("nivgen_stars"));
const planets = toObjs(await sheet("nivgen_planets"));
const byStar = new Map<string, Row[]>();
for (const p of planets) {
  const name = String(p.key).split("|")[0];
  if (!byStar.has(name)) byStar.set(name, []);
  byStar.get(name)!.push(p);
}
function comparable(name: string): Row[] {
  return (byStar.get(name) ?? []).filter((p) => p.orig_sect_def_oc && p.orig_sect_rand_oc);
}

const starNames = stars.map((s) => String(s.key)).filter((n) => comparable(n).length > 0).sort();
let target: string[];
const single = arg("star");
const count = arg("stars", "10")!;
if (single) target = starNames.filter((n) => n === single);
else if (count === "all") target = starNames;
else target = starNames.slice(0, Number(count));
if (target.length === 0) throw new Error("no stars selected");

const jobs = Number(arg("jobs", "6"));
const outPath = arg("out");
let resume: Record<string, unknown> = {};
if (outPath) {
  try { resume = JSON.parse(await Deno.readTextFile(outPath)); } catch { /* fresh */ }
}

console.log(`engine : ${ENGINE}${ENGINE === "lino" ? ` (${NIVLIN})` : ` (${NIVGEN})`}`);
console.log(`fields : ${FIELDS_ARG}`);
console.log(`stars  : ${target.length} of ${starNames.length}`);
console.log(`jobs   : ${jobs}\n`);

let totalChecks = 0, totalMismatch = 0, totalBodies = 0;
const results: Record<string, unknown> = { ...resume };
const queue = target.filter((n) => !results[n]);
const worker = async () => {
  for (;;) {
    const name = queue.shift();
    if (!name) return;
    const st = stars.find((s) => String(s.key) === name)!;
    const bodies = comparable(name);
    let checks = 0;
    const bad: string[] = [];
    for (const p of bodies) {
      const body = Number(p.body);
      const def = parseSector(run("sector", Number(st.x), Number(st.y), Number(st.z), body, 0, 60));
      const rand = parseSector(run("sector", Number(st.x), Number(st.y), Number(st.z), body, Number(p.rand_lon), Number(p.rand_lat)));
      const pairs: [string, string | undefined, unknown][] = [
        [`${p.key} def_hm`, def.hm, p.orig_sect_def_hm],
        [`${p.key} def_oc`, def.oc, p.orig_sect_def_oc],
        [`${p.key} rand_hm`, rand.hm, p.orig_sect_rand_hm],
        [`${p.key} rand_oc`, rand.oc, p.orig_sect_rand_oc],
      ];
      if (FIELDS_ARG === "all") {
        const defT = parseTex(run("surftex", Number(st.x), Number(st.y), Number(st.z), body, 0, 60));
        const randT = parseTex(run("surftex", Number(st.x), Number(st.y), Number(st.z), body, Number(p.rand_lon), Number(p.rand_lat)));
        pairs.push(
          [`${p.key} def_stex`, defT.stex, p.orig_sect_def_stex],
          [`${p.key} def_sky`, defT.sky, p.orig_sect_def_sky],
          [`${p.key} rand_stex`, randT.stex, p.orig_sect_rand_stex],
          [`${p.key} rand_sky`, randT.sky, p.orig_sect_rand_sky],
        );
      }
      for (const [label, got, want] of pairs) {
        if (want === undefined || want === null) continue;
        checks++;
        if (got !== String(want)) bad.push(`${label}: got ${got} want ${want}`);
      }
    }
    results[name] = { bodies: bodies.length, checks, mismatches: bad.length, detail: bad.slice(0, 6) };
    totalChecks += checks;
    totalMismatch += bad.length;
    totalBodies += bodies.length;
    if (outPath) await Deno.writeTextFile(outPath, JSON.stringify(results, null, 2));
    console.log(`${bad.length === 0 ? "OK  " : "FAIL"} ${name.padEnd(24)} bodies=${String(bodies.length).padStart(3)} checks=${String(checks).padStart(4)} mismatches=${bad.length}`);
    for (const b of bad.slice(0, 3)) console.log(`       ${b}`);
  }
};
await Promise.all(Array.from({ length: Math.min(jobs, queue.length || 1) }, worker));

const failedStars = Object.entries(results).filter(([, v]) => Number((v as { mismatches: number }).mismatches) > 0);
console.log(`\n=== SUMMARY (${ENGINE}, fields=${FIELDS_ARG}) ===`);
console.log(`stars=${target.length} bodies=${totalBodies} checks=${totalChecks} mismatches=${totalMismatch}`);
if (failedStars.length) {
  console.log("stars with mismatches:");
  for (const [name, v] of failedStars) console.log(`  ${name}: ${(v as { mismatches: number }).mismatches}`);
}
console.log(totalMismatch === 0 ? "RESULT: ALL MATCHED (no gap injected)" : `RESULT: ${totalMismatch} MISMATCHES`);
Deno.exit(totalMismatch === 0 ? 0 : 1);

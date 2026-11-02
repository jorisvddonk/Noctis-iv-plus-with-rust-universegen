#!/usr/bin/env -S deno run -A
/**
 * Manual validator for the derived farmalloc gap.
 *
 * Runs the rust engine WITHOUT -gap (so it uses derive_oc_gap() internally) and
 * compares the sector heightmap/objectchart hashes against the frozen orig
 * corpus in the SheetBot sheets. This is the check that says whether the port
 * can compute the heap header itself and stop depending on the C build.
 *
 * Usage:
 *   SHEETBOT_AUTH_APIKEY=... deno run -A tests/nivgen/tools/validate_gap.ts \
 *       --stars 10            # first N stars (alphabetical)
 *   ... --stars all           # every star in the sheet
 *   ... --star "MIDORI"       # a single star
 *   ... --stars 10 --jobs 8   # parallel engine runs
 *   ... --out results.json    # checkpoint file (resumable)
 *
 * Exit code is 0 only when every compared hash matched.
 */
const BASE = Deno.env.get("SHEETBOT_BASEURL") ?? "https://litterbox.moos.es";
const KEY = Deno.env.get("SHEETBOT_AUTH_APIKEY");
if (!KEY) {
  console.error("SHEETBOT_AUTH_APIKEY is required (it is in ~/.config/sheetbot/process-compose.yml)");
  Deno.exit(2);
}
const REPO = new URL("../../../", import.meta.url).pathname.replace(/\/$/, "");
const BIN = `${REPO}/tests/nivgen/target/release/nivgen`;

function arg(name: string, dflt?: string): string | undefined {
  const i = Deno.args.indexOf(`--${name}`);
  return i >= 0 ? Deno.args[i + 1] : dflt;
}

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

function sector(x: number, y: number, z: number, body: number, lon: number, lat: number) {
  const p = new Deno.Command(BIN, {
    args: ["sector", "-x", String(x), "-y", String(y), "-z", String(z), "-p", String(body),
      "-lon", String(lon), "-lat", String(lat)],
    cwd: REPO, stdout: "piped", stderr: "null",
  }).outputSync();
  const t = new TextDecoder().decode(p.stdout);
  const hm = t.match(/^heightmap\s+len=\d+\s+fnv=(\w+)/m);
  const oc = t.match(/^objectchart\s+len=\d+\s+fnv=(\w+)/m);
  if (!hm || !oc) return undefined;
  return { hm: hm[1], oc: oc[1] };
}

const stars = toObjs(await sheet("nivgen_stars"));
const planets = toObjs(await sheet("nivgen_planets"));
const byStar = new Map<string, Row[]>();
for (const p of planets) {
  const name = String(p.key).split("|")[0];
  if (!byStar.has(name)) byStar.set(name, []);
  byStar.get(name)!.push(p);
}

// Bodies that actually have orig sector data to compare against.
function comparable(name: string): Row[] {
  return (byStar.get(name) ?? []).filter((p) => p.orig_sect_def_oc && p.orig_sect_rand_oc);
}

const starNames = stars
  .map((s) => String(s.key))
  .filter((n) => comparable(n).length > 0)
  .sort();

let target: string[];
const single = arg("star");
const count = arg("stars", "10")!;
if (single) target = starNames.filter((n) => n === single);
else if (count === "all") target = starNames;
else target = starNames.slice(0, Number(count));
if (target.length === 0) throw new Error("no stars selected");

const jobs = Number(arg("jobs", "8"));
const outPath = arg("out");
const resume: Record<string, unknown> = {};
if (outPath) {
  try { Object.assign(resume, JSON.parse(await Deno.readTextFile(outPath))); } catch { /* fresh */ }
}

console.log(`repo   : ${REPO}`);
console.log(`engine : ${BIN}`);
console.log(`stars  : ${target.length} of ${starNames.length}`);
console.log(`jobs   : ${jobs}\n`);

let totalChecks = 0, totalMismatch = 0, totalBodies = 0;
const starResults: Record<string, unknown> = {};
const queue = target.filter((n) => !resume[n]);
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
      const def = sector(Number(st.x), Number(st.y), Number(st.z), body, 0, 60);
      const rand = sector(Number(st.x), Number(st.y), Number(st.z), body, Number(p.rand_lon), Number(p.rand_lat));
      if (!def || !rand) { bad.push(`${p.key}: engine error`); continue; }
      const pairs: [string, string, string][] = [
        [`${p.key} def_hm`, def.hm, String(p.orig_sect_def_hm)],
        [`${p.key} def_oc`, def.oc, String(p.orig_sect_def_oc)],
        [`${p.key} rand_hm`, rand.hm, String(p.orig_sect_rand_hm)],
        [`${p.key} rand_oc`, rand.oc, String(p.orig_sect_rand_oc)],
      ];
      for (const [label, got, want] of pairs) {
        checks++;
        if (got !== want) bad.push(`${label}: got ${got} want ${want} (gap ${p.orig_sect_rand_gap})`);
      }
    }
    starResults[name] = { bodies: bodies.length, checks, mismatches: bad.length, detail: bad.slice(0, 6) };
    totalChecks += checks;
    totalMismatch += bad.length;
    totalBodies += bodies.length;
    if (outPath) await Deno.writeTextFile(outPath, JSON.stringify({ ...resume, ...starResults }, null, 2));
    const mark = bad.length === 0 ? "OK  " : "FAIL";
    console.log(`${mark} ${name.padEnd(24)} bodies=${String(bodies.length).padStart(3)} checks=${String(checks).padStart(4)} mismatches=${bad.length}`);
    for (const b of bad.slice(0, 3)) console.log(`       ${b}`);
  }
};
await Promise.all(Array.from({ length: Math.min(jobs, queue.length || 1) }, worker));

console.log(`\n=== SUMMARY ===`);
console.log(`stars=${target.length} bodies=${totalBodies} checks=${totalChecks} mismatches=${totalMismatch}`);
console.log(totalMismatch === 0 ? "RESULT: ALL MATCHED (derived gap reproduces orig)" : `RESULT: ${totalMismatch} MISMATCHES`);
Deno.exit(totalMismatch === 0 ? 0 : 1);

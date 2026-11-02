#!/usr/bin/env -S deno run -A
/**
 * Re-captures `orig` sector hashes for specific bodies using the canonicalised
 * C harness (the one whose `build_surface` pins the far-heap header instead of
 * inheriting it from the binary's memory layout).
 *
 * Needed only for captures that were contaminated: when the harness processed
 * several bodies in one DOSBox session, the header of a later body reflected
 * heap state left by earlier ones, so its stored object chart is not
 * reproducible by any single canonical layout.
 *
 * Writes orig_sect_{def,rand}_{hm,oc,gap} for each body key given.
 *
 * Usage:
 *   SHEETBOT_AUTH_APIKEY=... deno run -A tests/nivgen/tools/recapture_orig.ts \
 *       "VENE'EREIB|20" "VENE'EREIB|21" --jobs 2
 */
const BASE = Deno.env.get("SHEETBOT_BASEURL") ?? "https://litterbox.moos.es";
const KEY = Deno.env.get("SHEETBOT_AUTH_APIKEY");
if (!KEY) { console.error("SHEETBOT_AUTH_APIKEY required"); Deno.exit(2); }
const REPO = new URL("../../../", import.meta.url).pathname.replace(/\/$/, "");
const TOOLCHAIN = `${REPO}/niv-toolchain/BCPP31`;
const EXE_DIR = `${REPO}/tests/.harness_build`;

const keys = Deno.args.filter((a) => !a.startsWith("--") && !/^\d+$/.test(a));
const jobsIdx = Deno.args.indexOf("--jobs");
const jobs = jobsIdx >= 0 ? Number(Deno.args[jobsIdx + 1]) : 2;
if (keys.length === 0) { console.error("give one or more body keys, e.g. \"VENE'EREIB|20\""); Deno.exit(2); }

const login = await fetch(`${BASE}/login`, {
  method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ apiKey: KEY }),
});
const token = (await login.json()).token as string;
const H = { authorization: `Bearer ${token}`, "content-type": "application/json" };
async function sheet(id: string) {
  for (let a = 0; a < 5; a++) {
    const r = await fetch(`${BASE}/sheets/${id}`, { headers: H });
    const t = await r.text();
    if (r.ok && t.trim().startsWith("{")) return JSON.parse(t) as { columns: { name: string }[]; rows: unknown[][] };
    await new Promise((res) => setTimeout(res, 1500));
  }
  throw new Error(`sheet ${id} unavailable`);
}
type Row = Record<string, unknown>;
const toObjs = (s: { columns: { name: string }[]; rows: unknown[][] }): Row[] =>
  s.rows.map((r) => { const o: Row = {}; s.columns.forEach((c, i) => { o[c.name] = r[i]; }); return o; });

const stars = new Map(toObjs(await sheet("nivgen_stars")).map((s) => [String(s.key), s]));
const planets = toObjs(await sheet("nivgen_planets"));

function cSector(x: number, y: number, z: number, body: number, lon: number, lat: number) {
  const work = Deno.makeTempDirSync({ prefix: "recap_" });
  try {
    Deno.writeTextFileSync(`${work}/RUN.BAT`,
      `NIVTEST.EXE sector -x ${x} -y ${y} -z ${z} -p ${body} -lon ${lon} -lat ${lat} -o D:\\SECT.OUT\r\nexit\r\n`);
    Deno.writeTextFileSync(`${work}/niv.conf`, [
      "[cpu]", "cputype=486", "cycles=max", "core=dynamic",
      "[dos]", "xms=true", "ems=true",
      "[sdl]", "startup_verbosity=quiet",
      "[autoexec]",
      `mount c ${TOOLCHAIN}`,
      `mount d ${work}`,
      `mount e ${EXE_DIR}`,
      "path C:\\BIN;E:\\",
      "d:",
      "copy E:\\NIVTEST.EXE D:\\NIVTEST.EXE >NUL",
      "call RUN.BAT",
      "exit",
    ].join("\r\n"));
    new Deno.Command("dosbox-x", {
      args: ["-conf", `${work}/niv.conf`, "-exit"], cwd: REPO,
      env: { SDL_VIDEODRIVER: "dummy", SDL_AUDIODRIVER: "dummy" },
      stdout: "null", stderr: "null",
    }).outputSync();
    const out = Deno.readTextFileSync(`${work}/SECT.OUT`);
    const pick = (re: RegExp) => { const m = out.match(re); return m ? m[1] : undefined; };
    return {
      hm: pick(/^heightmap\s+len=\d+\s+fnv=(\w+)/m),
      oc: pick(/^objectchart\s+len=\d+\s+fnv=(\w+)/m),
      gap: pick(/^gap\s+len=16\s+(\w+)/m),
    };
  } finally {
    try { Deno.removeSync(work, { recursive: true }); } catch { /* ignore */ }
  }
}

const queue = [...keys];
const worker = async () => {
  for (;;) {
    const key = queue.shift();
    if (!key) return;
    const p = planets.find((x) => String(x.key) === key);
    if (!p) { console.log(`MISSING sheet row for ${key}`); continue; }
    const st = stars.get(key.split("|")[0]);
    if (!st) { console.log(`MISSING star for ${key}`); continue; }
    const def = cSector(Number(st.x), Number(st.y), Number(st.z), Number(p.body), 0, 60);
    const rand = cSector(Number(st.x), Number(st.y), Number(st.z), Number(p.body), Number(p.rand_lon), Number(p.rand_lat));
    if (!def.hm || !rand.hm) { console.log(`ERROR harness output for ${key}`); continue; }
    const patch = {
      orig_sect_def_hm: def.hm, orig_sect_def_oc: def.oc, orig_sect_def_gap: def.gap,
      orig_sect_rand_hm: rand.hm, orig_sect_rand_oc: rand.oc, orig_sect_rand_gap: rand.gap,
    };
    let r: Response | undefined;
    for (let attempt = 0; attempt < 5; attempt++) {
      try {
        r = await fetch(`${BASE}/sheets/nivgen_planets/data`, {
          method: "POST", headers: H, body: JSON.stringify({ key, ...patch }),
        });
        if (r.ok) break;
      } catch {
        // transient — retry
      }
      await new Promise((res) => setTimeout(res, 1500));
    }
    if (!r) { console.log(`POST failed for ${key}`); continue; }
    const changed = ["def_oc", "rand_oc", "def_hm", "rand_hm"].filter((f) => {
      const [sect, field] = f.split("_");
      return String(p[`orig_sect_${sect}_${field}`]) !== String(patch[`orig_sect_${sect}_${field}` as keyof typeof patch]);
    });
    console.log(`${r.status === 200 ? "updated" : `HTTP ${r.status}`} ${key.padEnd(18)} gap=${String(rand.gap).slice(24)} changed=${changed.join(",") || "none"}`);
  }
};
await Promise.all(Array.from({ length: Math.min(jobs, queue.length) }, worker));

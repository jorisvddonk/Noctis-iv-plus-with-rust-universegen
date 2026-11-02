#!/usr/bin/env -S deno run -A
/**
 * Ground truth for bodies whose captured gap is not the derived constant.
 *
 * For each such body this runs the C harness ALONE in a fresh DOSBox process and
 * records the header it emits. That answers the question the corpus cannot:
 * is the outlier an intrinsic property of that body (reproduces solo), or is it
 * residue from the harness having processed other bodies in the same session
 * (i.e. contamination of the capture, not of the game)?
 *
 * Usage:
 *   SHEETBOT_AUTH_APIKEY=... deno run -A tests/nivgen/tools/probe_c_gap.ts \
 *       --jobs 3 --out /tmp/c_gap_outliers.json
 */
const BASE = Deno.env.get("SHEETBOT_BASEURL") ?? "https://litterbox.moos.es";
const KEY = Deno.env.get("SHEETBOT_AUTH_APIKEY");
if (!KEY) { console.error("SHEETBOT_AUTH_APIKEY required"); Deno.exit(2); }
const REPO = new URL("../../../", import.meta.url).pathname.replace(/\/$/, "");
const TOOLCHAIN = `${REPO}/niv-toolchain/BCPP31`;
const EXE_DIR = `${REPO}/tests/.harness_build`;
const CONSTANT = "000000000000000000000000C509E951";

function arg(name: string, dflt?: string): string | undefined {
  const i = Deno.args.indexOf(`--${name}`);
  return i >= 0 ? Deno.args[i + 1] : dflt;
}
const login = await fetch(`${BASE}/login`, {
  method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ apiKey: KEY }),
});
const token = (await login.json()).token as string;
const H = { authorization: `Bearer ${token}` };
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
const outliers = planets.filter((p) => String(p.orig_sect_rand_gap || "") !== CONSTANT);

function cSector(x: number, y: number, z: number, body: number, lon: number, lat: number) {
  const work = Deno.makeTempDirSync({ prefix: "cgap_" });
  try {
    Deno.mkdirSync(`${work}/DUMP`, { recursive: true });
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
    const p = new Deno.Command("dosbox-x", {
      args: ["-conf", `${work}/niv.conf`, "-exit"],
      cwd: REPO,
      env: { SDL_VIDEODRIVER: "dummy", SDL_AUDIODRIVER: "dummy" },
      stdout: "null", stderr: "null",
    }).outputSync();
    try {
      const out = Deno.readTextFileSync(`${work}/SECT.OUT`);
      const gap = out.match(/^gap\s+len=16\s+(\w+)/m);
      const oc = out.match(/^objectchart\s+len=\d+\s+fnv=(\w+)/m);
      return { gap: gap ? gap[1] : undefined, oc: oc ? oc[1] : undefined };
    } catch {
      return { gap: undefined, oc: undefined };
    }
  } finally {
    try { Deno.removeSync(work, { recursive: true }); } catch { /* ignore */ }
  }
}

const jobs = Number(arg("jobs", "3"));
const outPath = arg("out");
const results: Record<string, unknown> = {};
const queue = [...outliers];
let done = 0;
const worker = async () => {
  for (;;) {
    const p = queue.shift();
    if (!p) return;
    const st = stars.get(String(p.key).split("|")[0]);
    if (!st) continue;
    const got = cSector(Number(st.x), Number(st.y), Number(st.z), Number(p.body),
      Number(p.rand_lon), Number(p.rand_lat));
    const verdict = !got.gap ? "ERROR"
      : got.gap === String(p.orig_sect_rand_gap) ? "CANONICAL (solo reproduces stored gap)"
      : got.gap === CONSTANT ? "CONTAMINATED (solo gives the constant)"
      : "OTHER";
    results[String(p.key)] = {
      storedGap: p.orig_sect_rand_gap, soloGap: got.gap,
      storedOc: p.orig_sect_rand_oc, soloOc: got.oc, verdict,
    };
    done++;
    console.log(`${verdict.padEnd(38)} ${String(p.key).padEnd(22)} stored=${String(p.orig_sect_rand_gap).slice(24)} solo=${String(got.gap).slice(24)} (${done}/${outliers.length})`);
    if (outPath) await Deno.writeTextFile(outPath, JSON.stringify(results, null, 2));
  }
};
await Promise.all(Array.from({ length: Math.min(jobs, queue.length) }, worker));

const tally: Record<string, number> = {};
Object.values(results).forEach((r) => {
  const v = String((r as Record<string, unknown>).verdict).split(" ")[0];
  tally[v] = (tally[v] || 0) + 1;
});
console.log(`\n=== ${outliers.length} outlier bodies ===`);
console.log(JSON.stringify(tally));

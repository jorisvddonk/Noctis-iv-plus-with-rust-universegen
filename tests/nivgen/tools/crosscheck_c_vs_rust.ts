#!/usr/bin/env -S deno run -A
/**
 * Independent cross-check: runs the C harness and the rust port on the same
 * bodies, live, and compares sector hashes. Nothing here trusts the SheetBot
 * corpus, so it validates both the derived gap and the corpus at once.
 *
 * The C build used is the canonicalised one (build_surface pins the far-heap
 * header), so this is the reference definition of "correct".
 *
 * Usage:
 *   deno run -A tests/nivgen/tools/crosscheck_c_vs_rust.ts --stars 6 --bodies 2
 */
const REPO = new URL("../../../", import.meta.url).pathname.replace(/\/$/, "");
const BIN = `${REPO}/tests/nivgen/target/release/nivgen`;
const TOOLCHAIN = `${REPO}/niv-toolchain/BCPP31`;
const EXE_DIR = `${REPO}/tests/.harness_build`;

function arg(name: string, dflt: string): string {
  const i = Deno.args.indexOf(`--${name}`);
  return i >= 0 ? Deno.args[i + 1] : dflt;
}

// Hand-picked fixtures: a spread of planet types/moons plus bodies that used to
// disagree before the gap derivation and before the OOB-write fix.
const FIXTURES: { label: string; x: number; y: number; z: number; body: number; lon: number; lat: number }[] = [
  { label: "MIDORI|1 (majority gap)", x: -379936080, y: -1443960, z: 54357502, body: 1, lon: 43, lat: 52 },
  { label: "ANDROI|6 (was 0000E900)", x: -280275088, y: -1411417, z: 54856035, body: 6, lon: 186, lat: 69 },
  { label: "NOCHUT|36 (was C5000051)", x: -1521346320, y: -1701020, z: -1370627468, body: 36, lon: 13, lat: 103 },
  { label: "TUCANO|9 (was 0000E951)", x: -200329008, y: -1386821, z: 54950436, body: 9, lon: 218, lat: 115 },
  { label: "VENE'EREIB|20 (re-captured)", x: -1419635728, y: -1063909, z: -1280553221, body: 20, lon: 21, lat: 33 },
  { label: "HIGHWAY 4|1 (was 00090000)", x: -20313648, y: -5356870, z: 37871746, body: 1, lon: 167, lat: 19 },
  { label: "IKTAYEL|9 (was C5000000)", x: -1996647728, y: 57093, z: 1063045, body: 9, lon: 32, lat: 75 },
  { label: "GALARETKA|6 (was all-zero)", x: -107235952, y: 25138811, z: -1828589, body: 6, lon: 216, lat: 32 },
];

function rustSector(f: typeof FIXTURES[number], lon: number, lat: number) {
  const p = new Deno.Command(BIN, {
    args: ["sector", "-x", String(f.x), "-y", String(f.y), "-z", String(f.z), "-p", String(f.body),
      "-lon", String(lon), "-lat", String(lat)],
    cwd: REPO, stdout: "piped", stderr: "null",
  }).outputSync();
  const t = new TextDecoder().decode(p.stdout);
  return {
    hm: t.match(/^heightmap\s+len=\d+\s+fnv=(\w+)/m)?.[1],
    oc: t.match(/^objectchart\s+len=\d+\s+fnv=(\w+)/m)?.[1],
  };
}

function cSector(f: typeof FIXTURES[number], lon: number, lat: number) {
  const work = Deno.makeTempDirSync({ prefix: "xcheck_" });
  try {
    Deno.writeTextFileSync(`${work}/RUN.BAT`,
      `NIVTEST.EXE sector -x ${f.x} -y ${f.y} -z ${f.z} -p ${f.body} -lon ${lon} -lat ${lat} -o D:\\SECT.OUT\r\nexit\r\n`);
    Deno.writeTextFileSync(`${work}/niv.conf`, [
      "[cpu]", "cputype=486", "cycles=max", "core=dynamic",
      "[dos]", "xms=true", "ems=true",
      "[sdl]", "startup_verbosity=quiet",
      "[autoexec]",
      `mount c ${TOOLCHAIN}`, `mount d ${work}`, `mount e ${EXE_DIR}`,
      "path C:\\BIN;E:\\", "d:",
      "copy E:\\NIVTEST.EXE D:\\NIVTEST.EXE >NUL", "call RUN.BAT", "exit",
    ].join("\r\n"));
    new Deno.Command("dosbox-x", {
      args: ["-conf", `${work}/niv.conf`, "-exit"], cwd: REPO,
      env: { SDL_VIDEODRIVER: "dummy", SDL_AUDIODRIVER: "dummy" },
      stdout: "null", stderr: "null",
    }).outputSync();
    const t = Deno.readTextFileSync(`${work}/SECT.OUT`);
    return {
      hm: t.match(/^heightmap\s+len=\d+\s+fnv=(\w+)/m)?.[1],
      oc: t.match(/^objectchart\s+len=\d+\s+fnv=(\w+)/m)?.[1],
      gap: t.match(/^gap\s+len=16\s+(\w+)/m)?.[1],
    };
  } finally {
    try { Deno.removeSync(work, { recursive: true }); } catch { /* ignore */ }
  }
}

const jobs = Number(arg("jobs", "2"));
let failures = 0, checks = 0;
const queue = [...FIXTURES];
const worker = async () => {
  for (;;) {
    const f = queue.shift();
    if (!f) return;
    for (const [tag, lon, lat] of [["def", 0, 60], ["rand", f.lon, f.lat]] as [string, number, number][]) {
      const c = cSector(f, lon, lat);
      const r = rustSector(f, lon, lat);
      for (const field of ["hm", "oc"] as const) {
        checks++;
        const ok = c[field] === r[field];
        if (!ok) failures++;
        console.log(`${ok ? "ok  " : "FAIL"} ${f.label.padEnd(30)} ${tag}_${field}  C=${c[field]} rust=${r[field]}${tag === "rand" && field === "oc" ? `  (C gap ${c.gap?.slice(24)})` : ""}`);
      }
    }
  }
};
await Promise.all(Array.from({ length: Math.min(jobs, queue.length) }, worker));
console.log(`\n${checks} checks, ${failures} mismatches`);
console.log(failures === 0 ? "RESULT: C and rust agree on every fixture" : "RESULT: MISMATCHES");
Deno.exit(failures === 0 ? 0 : 1);

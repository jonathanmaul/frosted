// Rebuild the local sculpture: node scripts/generate-frost-model.mjs (docs/DESIGN.md §13).
import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import * as THREE from "three";
import { ConvexGeometry } from "three/addons/geometries/ConvexGeometry.js";
import { GLTFExporter } from "three/addons/exporters/GLTFExporter.js";

// GLTFExporter expects the browser FileReader API; Node supplies Blob.arrayBuffer instead.
globalThis.FileReader = class {
  readAsArrayBuffer(blob) { blob.arrayBuffer().then((buffer) => { this.result = buffer; this.onloadend?.(); }); }
};
const sculpture = new THREE.Group(); sculpture.name = "FrostLedger";
const ice = new THREE.MeshPhysicalMaterial({ name: "OpticalIce", color: "#DCEBFF", transmission: 1, thickness: 1.15, roughness: 0.22, ior: 1.31, metalness: 0, clearcoat: 1, clearcoatRoughness: 0.08 });
const facet = new THREE.MeshPhysicalMaterial({ name: "FracturedIce", color: "#DCEBFF", transmission: 0.8, thickness: 0.32, roughness: 0.16, ior: 1.31, clearcoat: 1 });
const etched = new THREE.MeshStandardMaterial({ name: "EtchedLedger", color: "#8FA8C8", roughness: 0.4, metalness: 0.7, transparent: true, opacity: 0.4 });
function mesh(name, geometry, material, position = [0, 0, 0], rotation = [0, 0, 0]) {
  const object = new THREE.Mesh(geometry, material); object.name = name; object.position.set(...position); object.rotation.set(...rotation); sculpture.add(object); return object;
}
const outline = [[-0.96, -1.46], [0.69, -1.5], [1.14, -1.16], [1.19, 0.86], [0.78, 1.48], [-0.58, 1.53], [-1.15, 1.1], [-1.22, -0.91]];
const shape = new THREE.Shape(outline.map(([x, y]) => new THREE.Vector2(x, y)));
const shell = new THREE.ExtrudeGeometry(shape, { depth: 0.92, steps: 1, bevelEnabled: true, bevelSegments: 3, bevelSize: 0.12, bevelThickness: 0.12, curveSegments: 1 });
shell.translate(0, 0, -0.46);
mesh("IceShell", shell, ice);
// Large crystalline inclusions, fused to the rim rather than a particle field.
const shards = [
  [[-1.05, -1.22, -0.3], [-1.35, -0.7, -0.16], [-1.29, 0.7, -0.22], [-0.98, 1.15, -0.3], [-0.93, -1.1, 0.44], [-1.08, -0.35, 0.57], [-1.04, 0.75, 0.49], [-0.83, 1.21, 0.34]],
  [[0.8, -1.31, -0.3], [1.22, -0.96, -0.26], [1.31, 0.61, -0.22], [0.95, 1.22, -0.3], [0.81, -1.22, 0.43], [1.14, -0.63, 0.58], [1.12, 0.69, 0.56], [0.78, 1.27, 0.34]],
  [[-0.81, 1.31, -0.25], [-0.3, 1.66, -0.12], [0.61, 1.62, -0.19], [1.01, 1.12, -0.25], [-0.76, 1.3, 0.4], [-0.22, 1.51, 0.58], [0.53, 1.45, 0.55], [0.8, 1.19, 0.42]],
];
shards.forEach((points, i) => mesh(`RimFacet_${i}`, new ConvexGeometry(points.map((p) => new THREE.Vector3(...p))), facet));
// Suspended folios and close-set cut lines give the ice an auditable interior.
for (let i = 0; i < 3; i++) {
  const folio = new THREE.BoxGeometry(1.9 - i * 0.08, 2.48 - i * 0.08, 0.012);
  const edges = new THREE.EdgesGeometry(folio);
  const line = new THREE.LineSegments(edges, new THREE.LineBasicMaterial({ color: "#DCEBFF", transparent: true, opacity: 0.2 }));
  line.name = `SuspendedFolio_${i}`; line.position.set(0, 0, -0.3 + i * 0.18); sculpture.add(line);
}
for (let i = 0; i < 17; i++) {
  const y = -1.16 + i * 0.145;
  mesh(`Calibration_${i}`, new THREE.BoxGeometry(i % 4 === 0 ? 0.16 : 0.07, 0.012, 0.012), etched, [-0.94, y, 0.586]);
}
for (const y of [-1.27, -1.22, 1.22, 1.27]) mesh(`EtchedRail_${y}`, new THREE.BoxGeometry(1.63, 0.009, 0.009), etched, [0, y, 0.586]);
const fractures = [
  [-1.06, 0.87, 0.59, -0.74, 0.47, 0.59, -0.74, 0.47, 0.59, -0.86, 0.1, 0.59, -0.74, 0.47, 0.59, -0.48, 0.35, 0.59],
  [1.03, -0.91, 0.59, 0.75, -0.54, 0.59, 0.75, -0.54, 0.59, 0.84, -0.17, 0.59],
];
fractures.forEach((points, i) => {
  const geometry = new THREE.BufferGeometry(); geometry.setAttribute("position", new THREE.Float32BufferAttribute(points, 3));
  const line = new THREE.LineSegments(geometry, new THREE.LineBasicMaterial({ color: "#DCEBFF", transparent: true, opacity: 0.16 })); line.name = `Fissure_${i}`; sculpture.add(line);
});
const glb = await new GLTFExporter().parseAsync(sculpture, { binary: true, onlyVisible: true });
const bytes = Buffer.from(glb);
assert.equal(bytes.readUInt32LE(0), 0x46546c67); assert.equal(bytes.readUInt32LE(4), 2);
assert.equal(bytes.readUInt32LE(8), bytes.length);
const jsonLength = bytes.readUInt32LE(12);
const json = JSON.parse(bytes.subarray(20, 20 + jsonLength).toString());
assert(json.nodes.some((node) => node.name === "IceShell"));
assert(!json.buffers.some((buffer) => buffer.uri));
assert(bytes.length < 500_000, "Keep the hero model below 500 KB");
await mkdir(new URL("../public/models/", import.meta.url), { recursive: true });
await writeFile(new URL("../public/models/frost-ledger.glb", import.meta.url), bytes);
console.log(`frost-ledger.glb: ${bytes.length.toLocaleString()} bytes, ${json.meshes.length} meshes, self-contained GLB 2.0`);

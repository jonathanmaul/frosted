"use client";

// Small ambient scenes for page headers (docs/decisions/0019-paper-redesign.md). Procedural; transparent canvas.
import { Edges, Environment, Float, Lightformer } from "@react-three/drei";
import { Canvas, useFrame } from "@react-three/fiber";
import { useMemo, useRef } from "react";
import * as THREE from "three";

export type SceneKind = "crystal" | "towers" | "chain";
export type SceneProps = { kind: SceneKind; active: boolean; yes?: number | null; no?: number | null; onFailure?: () => void };

const INK = "#0A1424", CORE = "#2B59FF", ICE = "#93B0DC", ACCENT = "#FF4F1F", YES = "#0B7A55", NO = "#C41F37", MUTED = "#4B6078";
const rand = (seed: number) => { const x = Math.sin(seed * 127.1) * 43758.5453; return x - Math.floor(x); };

function Orbiters({ count = 18, radius = 2.2 }: { count?: number; radius?: number }) {
  const ref = useRef<THREE.InstancedMesh>(null);
  const seeds = useMemo(() => Array.from({ length: count }, (_, i) => ({ a: rand(i + 1) * Math.PI * 2, r: radius + rand(i + 50) * 1.1, y: (rand(i + 90) - 0.5) * 2, s: 0.05 + rand(i + 7) * 0.11, sp: 0.1 + rand(i + 3) * 0.25 })), [count, radius]);
  const dummy = useMemo(() => new THREE.Object3D(), []);
  useFrame(({ clock }) => {
    const t = clock.elapsedTime;
    seeds.forEach((s, i) => {
      const a = s.a + t * s.sp;
      dummy.position.set(Math.cos(a) * s.r, s.y + Math.sin(t * 0.7 + i) * 0.1, Math.sin(a) * s.r);
      dummy.rotation.set(t * s.sp * 2, t * s.sp, i); dummy.scale.setScalar(s.s); dummy.updateMatrix();
      ref.current?.setMatrixAt(i, dummy.matrix);
    });
    if (ref.current) ref.current.instanceMatrix.needsUpdate = true;
  });
  return <instancedMesh ref={ref} args={[undefined, undefined, count]} frustumCulled={false}><tetrahedronGeometry args={[1, 0]} /><meshStandardMaterial color={ACCENT} emissive={ACCENT} emissiveIntensity={0.3} roughness={0.35} flatShading /></instancedMesh>;
}

function Crystal() {
  const group = useRef<THREE.Group>(null);
  const core = useRef<THREE.Mesh>(null);
  useFrame(({ clock, pointer }) => {
    const t = clock.elapsedTime;
    if (group.current) { group.current.rotation.y = t * 0.2 + pointer.x * 0.5; group.current.rotation.x = Math.sin(t * 0.3) * 0.12 - pointer.y * 0.3; }
    if (core.current) core.current.scale.setScalar(0.62 + Math.sin(t * 1.6) * 0.05);
  });
  return <Float speed={1.2} floatIntensity={0.5} rotationIntensity={0.1}><group ref={group}>
    <mesh><icosahedronGeometry args={[1.3, 1]} /><meshPhysicalMaterial color={ICE} flatShading roughness={0.1} transmission={0.5} thickness={1.2} ior={1.45} iridescence={1} clearcoat={1} transparent opacity={0.92} /><Edges color={CORE} threshold={12} transparent opacity={0.75} /></mesh>
    <mesh ref={core}><octahedronGeometry args={[1, 0]} /><meshBasicMaterial color={CORE} toneMapped={false} /></mesh>
    <pointLight color={CORE} intensity={12} distance={8} />
  </group></Float>;
}

function Towers({ yes, no }: Pick<SceneProps, "yes" | "no">) {
  const bars = useRef<THREE.Group>(null);
  useFrame(({ clock }, dt) => {
    const t = clock.elapsedTime;
    bars.current?.children.forEach((bar, i) => {
      const p = i === 0 ? yes : no;
      const target = Math.max(p ?? 0.5, 0.05) * 3.2 + Math.sin(t * 0.9 + i * 2) * 0.06;
      bar.scale.y = THREE.MathUtils.damp(bar.scale.y, target, 4, dt);
      bar.position.y = -1.2 + bar.scale.y / 2;
    });
    if (bars.current) bars.current.rotation.y = Math.sin(t * 0.25) * 0.35 - 0.3;
  });
  return <group>
    <group ref={bars}>{[YES, NO].map((c, i) => <mesh key={c} position={[i === 0 ? -0.8 : 0.8, 0, 0]}><boxGeometry args={[1.1, 1, 1.1]} /><meshPhysicalMaterial color={c} roughness={0.25} clearcoat={1} transparent opacity={0.9} /><Edges color={INK} transparent opacity={0.3} /></mesh>)}</group>
    <mesh rotation-x={Math.PI / 2} position={[0, -1.21, 0]}><ringGeometry args={[2.1, 2.14, 64]} /><meshBasicMaterial color={MUTED} transparent opacity={0.5} side={THREE.DoubleSide} /></mesh>
  </group>;
}

function Chain() {
  const group = useRef<THREE.Group>(null);
  const cubes = useRef<THREE.Group>(null);
  useFrame(({ clock }) => {
    const t = clock.elapsedTime;
    if (group.current) group.current.rotation.y = Math.sin(t * 0.3) * 0.5 - 0.5;
    cubes.current?.children.forEach((c, i) => { const pulse = Math.max(0, Math.sin(t * 1.4 - i * 0.9)); c.scale.setScalar(0.85 + pulse * 0.22); });
  });
  return <group ref={group}>
    <group ref={cubes}>{[0, 1, 2, 3, 4].map((i) => <mesh key={i} position={[(i - 2) * 1.05, Math.sin(i * 1.3) * 0.25, 0]}>
      <boxGeometry args={[0.7, 0.7, 0.7]} /><meshPhysicalMaterial color={i === 4 ? ACCENT : ICE} roughness={0.15} clearcoat={1} transparent opacity={0.92} /><Edges color={i === 4 ? INK : CORE} transparent opacity={0.8} />
    </mesh>)}</group>
    <mesh position={[0, 0, 0]}><boxGeometry args={[4.2, 0.02, 0.02]} /><meshBasicMaterial color={MUTED} transparent opacity={0.5} /></mesh>
  </group>;
}

export default function Scene({ kind, active, yes = null, no = null, onFailure }: SceneProps) {
  return <Canvas frameloop={active ? "always" : "never"} dpr={[1, 1.5]} camera={{ position: [0, 0.4, kind === "chain" ? 5.6 : 6], fov: 38 }} gl={{ antialias: true, alpha: true, powerPreference: "low-power" }}
    onCreated={({ gl }) => { gl.setClearColor(0x000000, 0); gl.domElement.addEventListener("webglcontextlost", () => onFailure?.(), { once: true }); }}>
    <ambientLight intensity={1.3} />
    <directionalLight position={[-3, 5, 4]} intensity={2.2} />
    <Environment resolution={64} frames={1}>
      <Lightformer form="rect" intensity={4} position={[-3, 2, 3]} scale={[2, 6, 1]} target={[0, 0, 0]} />
      <Lightformer form="rect" intensity={3} position={[3, 1, 2]} scale={[1, 5, 1]} target={[0, 0, 0]} />
      <Lightformer form="rect" intensity={2.5} position={[0, 4, 0]} scale={[6, 1, 1]} target={[0, 0, 0]} />
    </Environment>
    {kind === "crystal" && <Crystal />}
    {kind === "towers" && <Towers yes={yes} no={no} />}
    {kind === "chain" && <Chain />}
    <Orbiters count={kind === "chain" ? 8 : 16} radius={kind === "chain" ? 2.6 : 2.2} />
  </Canvas>;
}

"use client";

// Paper direction (docs/decisions/0019-paper-redesign.md): procedural crystal on a light ground, no external assets.
import { Edges, Environment, Lightformer } from "@react-three/drei";
import { Canvas, useFrame } from "@react-three/fiber";
import { useEffect, useMemo, useRef, type RefObject } from "react";
import * as THREE from "three";

import { journeyFrame, type JourneyState } from "@/lib/journey";
import { formatPercent } from "@/lib/api";

export type SceneProps = { yes: number | null; no: number | null; step: number; journey: RefObject<JourneyState>; active?: boolean; onReady?: () => void; onFailure?: () => void };

const PAPER = "#EEF3F8", INK = "#0A1424", MUTED = "#4B6078", CORE = "#2B59FF", ICE = "#93B0DC", ACCENT = "#FF4F1F", YES = "#0B7A55", NO = "#C41F37";
const rand = (seed: number) => { const x = Math.sin(seed * 127.1) * 43758.5453; return x - Math.floor(x); };

function Silhouettes({ step, journey }: Pick<SceneProps, "step" | "journey">) {
  const group = useRef<THREE.Group>(null);
  const texture = useMemo(() => {
    const c = document.createElement("canvas");
    c.width = 96; c.height = 192;
    const ctx = c.getContext("2d")!;
    // Blur is baked into the texture: cursor movement can never reveal an identity.
    ctx.filter = "blur(13px)";
    ctx.fillStyle = MUTED;
    ctx.beginPath(); ctx.roundRect(31, 26, 34, 136, 17); ctx.fill();
    return new THREE.CanvasTexture(c);
  }, []);
  useEffect(() => () => texture.dispose(), [texture]);
  useFrame(({ clock }) => {
    const t = clock.getElapsedTime();
    const p = journey.current.active ? journey.current.progress : 0;
    const spread = THREE.MathUtils.smoothstep(p, 0.16, 0.55);
    if (group.current) { group.current.position.z = -1.8 - spread * 14; group.current.scale.setScalar(1 + spread * 1.8); }
    group.current?.children.forEach((child, i) => {
      child.position.y = Math.sin(t * 0.19 + i * 1.7) * 0.3 + (i % 2 ? 0.55 : -0.35);
      child.position.x = -1.8 + i * 0.6 + Math.sin(t * 0.12 + i) * 0.1;
    });
  });
  return <group ref={group} position={[0, 0, -1.8]}>{Array.from({ length: 7 }, (_, i) => <sprite key={i} scale={[0.55, 1.1, 1]}><spriteMaterial map={texture} transparent opacity={step === 3 ? 0.6 : 0.3} depthWrite={false} /></sprite>)}</group>;
}

function OddsLabel({ yes, no }: Pick<SceneProps, "yes" | "no">) {
  const texture = useMemo(() => {
    const c = document.createElement("canvas"); c.width = 768; c.height = 256;
    const ctx = c.getContext("2d")!;
    ctx.font = '500 46px "IBM Plex Mono", monospace'; ctx.textAlign = "center";
    ctx.fillStyle = yes === null ? MUTED : YES; ctx.fillText(`YES  ${formatPercent(yes)}`, 196, 120);
    ctx.fillStyle = no === null ? MUTED : NO; ctx.fillText(`NO  ${formatPercent(no)}`, 572, 120);
    const tex = new THREE.CanvasTexture(c); tex.colorSpace = THREE.SRGBColorSpace;
    return tex;
  }, [yes, no]);
  useEffect(() => () => texture.dispose(), [texture]);
  return <mesh position={[0, -0.94, 0.3]}><planeGeometry args={[2, 0.67]} /><meshBasicMaterial map={texture} transparent depthWrite={false} /></mesh>;
}

function OddsBars({ yes, no, step }: Pick<SceneProps, "yes" | "no" | "step">) {
  const bars = useRef<THREE.Group>(null);
  const known = yes !== null && no !== null;
  useFrame((_, delta) => {
    bars.current?.children.forEach((bar, i) => {
      const probability = i === 0 ? yes : no;
      const target = step === 1 ? 0.22 : Math.max(probability ?? 0.5, 0.03) * 1.95;
      bar.scale.y = THREE.MathUtils.damp(bar.scale.y, target, 5, delta);
      bar.position.y = -0.64 + bar.scale.y / 2;
    });
  });
  return <><group ref={bars}>{[YES, NO].map((color, i) => <mesh key={color} position={[i === 0 ? -0.42 : 0.42, 0, 0]}><boxGeometry args={[0.48, 1, 0.36]} /><meshPhysicalMaterial color={known ? color : MUTED} roughness={0.25} metalness={0.1} clearcoat={1} transparent opacity={known ? 0.92 : 0.4} /><Edges color={INK} transparent opacity={0.35} /></mesh>)}</group><OddsLabel yes={yes} no={no} /></>;
}

function Shards({ journey }: Pick<SceneProps, "journey">) {
  const ref = useRef<THREE.InstancedMesh>(null);
  const COUNT = 30;
  const seeds = useMemo(() => Array.from({ length: COUNT }, (_, i) => ({ a: rand(i + 1) * Math.PI * 2, r: 2.3 + rand(i + 50) * 1.6, y: (rand(i + 90) - 0.5) * 2.4, s: 0.05 + rand(i + 7) * 0.13, sp: 0.1 + rand(i + 3) * 0.28 })), []);
  const dummy = useMemo(() => new THREE.Object3D(), []);
  useFrame(({ clock }) => {
    const t = clock.elapsedTime;
    const p = journey.current.active ? journey.current.progress : 0;
    const converge = THREE.MathUtils.smoothstep(p, 0.76, 0.94); // payout: shards merge back toward the centre
    seeds.forEach((s, i) => {
      const a = s.a + t * s.sp;
      const r = s.r * (1 - converge * 0.7);
      dummy.position.set(Math.cos(a) * r, s.y * (1 - converge * 0.6) + Math.sin(t * 0.7 + i) * 0.1, Math.sin(a) * r - 1.8 - THREE.MathUtils.smoothstep(p, 0.16, 0.55) * 2);
      dummy.rotation.set(t * s.sp * 2, t * s.sp, i);
      dummy.scale.setScalar(s.s);
      dummy.updateMatrix();
      ref.current?.setMatrixAt(i, dummy.matrix);
    });
    if (ref.current) ref.current.instanceMatrix.needsUpdate = true;
  });
  return <instancedMesh ref={ref} args={[undefined, undefined, COUNT]} frustumCulled={false}><tetrahedronGeometry args={[1, 0]} /><meshStandardMaterial color={ACCENT} emissive={ACCENT} emissiveIntensity={0.35} roughness={0.35} flatShading /></instancedMesh>;
}

function Snow() {
  const ref = useRef<THREE.Points>(null);
  const N = 500;
  const positions = useMemo(() => { const a = new Float32Array(N * 3); for (let i = 0; i < N; i++) { a[i * 3] = (rand(i) - 0.5) * 14; a[i * 3 + 1] = rand(i + 300) * 8 - 2; a[i * 3 + 2] = (rand(i + 600) - 0.5) * 10 - 2; } return a; }, []);
  useFrame(({ pointer }, dt) => {
    const geo = ref.current?.geometry.attributes.position as THREE.BufferAttribute | undefined;
    if (!geo) return;
    const px = pointer.x * 5, py = pointer.y * 2.5;
    for (let i = 0; i < N; i++) {
      let y = geo.getY(i) - dt * (0.25 + rand(i + 9) * 0.35);
      let x = geo.getX(i);
      const dx = x - px, dy = y - py, d2 = dx * dx + dy * dy;
      if (d2 < 1.6) x += (dx / (Math.sqrt(d2) + 0.01)) * dt * 2.2; // the cursor pushes flakes aside
      if (y < -2) y = 6;
      geo.setXY(i, x, y);
    }
    geo.needsUpdate = true;
  });
  return <points ref={ref} frustumCulled={false}><bufferGeometry><bufferAttribute attach="attributes-position" args={[positions, 3]} /></bufferGeometry><pointsMaterial color={CORE} size={0.04} sizeAttenuation transparent opacity={0.7} depthWrite={false} /></points>;
}

function Specimen({ yes, no, step, journey }: SceneProps) {
  const group = useRef<THREE.Group>(null);
  const core = useRef<THREE.Mesh>(null);
  useFrame(({ clock, pointer }, delta) => {
    const t = clock.getElapsedTime();
    const alignment = journey.current.active ? THREE.MathUtils.smoothstep(journey.current.progress, 0.04, 0.17) : 0;
    if (group.current) {
      group.current.rotation.y = THREE.MathUtils.damp(group.current.rotation.y, t * 0.16 * (1 - alignment) + pointer.x * 0.4, 3, delta);
      group.current.rotation.x = THREE.MathUtils.damp(group.current.rotation.x, Math.sin(t * 0.3) * 0.12 - pointer.y * 0.25, 3, delta);
      group.current.position.y = Math.sin(t * 0.5) * 0.08;
    }
    if (core.current) core.current.scale.setScalar(0.62 + Math.sin(t * 1.6) * 0.05);
  });
  return <>
  <group ref={group}>
    <mesh>
      <icosahedronGeometry args={[1.35, 1]} />
      <meshPhysicalMaterial color={ICE} flatShading roughness={0.1} metalness={0.1} transmission={0.5} thickness={1.2} ior={1.45} iridescence={1} iridescenceIOR={1.6} clearcoat={1} transparent opacity={0.92} envMapIntensity={1.3} />
      <Edges color={CORE} threshold={12} transparent opacity={0.75} />
    </mesh>
    <mesh ref={core}><octahedronGeometry args={[1, 0]} /><meshBasicMaterial color={CORE} toneMapped={false} /></mesh>
    <pointLight color={CORE} intensity={14} distance={8} />
  </group>
  {/* Outside the spinning group so the YES/NO labels always face the camera. */}
  <group position={[1.9, -0.5, 1.3]} scale={step === 1 ? 0.7 : 0.8}><OddsBars yes={yes} no={no} step={step} /></group>
  </>;
}

/** The camera traverses the crystal into a ledger world; progress never triggers a React render per frame. */
function JourneyCamera({ journey }: Pick<SceneProps, "journey">) {
  useFrame(({ camera, size }) => {
    const p = journey.current.progress;
    const { position, target } = journeyFrame(p);
    const compact = size.width < 640;
    const approach = compact ? 3 * (1 - THREE.MathUtils.smoothstep(p, 0, 0.18)) : 0;
    const lift = compact ? 0.15 * THREE.MathUtils.smoothstep(p, 0.14, 0.32) : 0;
    camera.position.set(position[0], position[1], position[2] + approach);
    camera.lookAt(target[0], target[1] - lift, target[2]);
  });
  return null;
}

function LedgerWorld({ yes, no, journey }: Pick<SceneProps, "yes" | "no" | "journey">) {
  const world = useRef<THREE.Group>(null);
  const payout = useRef<THREE.Group>(null);
  const publicOdds = useRef<THREE.Group>(null);
  const label = useMemo(() => {
    const canvas = document.createElement("canvas"); canvas.width = 1024; canvas.height = 256;
    const ctx = canvas.getContext("2d")!;
    ctx.fillStyle = MUTED; ctx.font = '400 28px "IBM Plex Mono"';
    ctx.fillText("GLASS ESCROW / PUBLIC VIEWING KEY", 36, 64);
    ctx.fillStyle = INK; ctx.font = '400 40px "IBM Plex Mono"';
    ctx.fillText("AMOUNT     OUTCOME     BLOCK", 36, 144);
    ctx.fillStyle = MUTED; ctx.fillText("SENDER     ░▒▓░▒▓░▒▓░▒▓░▒▓", 36, 216);
    const texture = new THREE.CanvasTexture(canvas); texture.colorSpace = THREE.SRGBColorSpace;
    return texture;
  }, []);
  useEffect(() => () => label.dispose(), [label]);
  useFrame(() => {
    const p = journey.current.progress;
    if (world.current) world.current.visible = journey.current.active && p > 0.08;
    if (payout.current) payout.current.visible = p > 0.7;
    if (publicOdds.current) publicOdds.current.visible = p > 0.18 && p < 0.6;
    payout.current?.children.forEach((child, i) => {
      const merge = THREE.MathUtils.smoothstep(p, 0.76, 0.94);
      child.position.x = (i - 1) * (1.55 - merge * 0.75);
      child.position.y = Math.sin(i * 2.1) * 0.4 * (1 - merge);
      child.rotation.z = (i - 1) * 0.16 * (1 - merge);
    });
  });
  return <group ref={world} visible={false}>
    <gridHelper args={[70, 35, MUTED, ICE]} position={[0, -2.1, -18]} material-transparent material-opacity={0.45} />
    {[0, 1, 2, 3].map((row) => <group key={row} position={[0, 0, -4 - row * 5.5]}>
      {[-1, 1].map((side) => <group key={side} position={[side * 3.7, 0.2, 0]} rotation={[0, side * -0.3, side * 0.035]}>
        <mesh><boxGeometry args={[2.8, 3.9, 0.035]} /><meshPhysicalMaterial color="#FFFFFF" metalness={0.05} roughness={0.3} transparent opacity={0.55} /><Edges color={MUTED} transparent opacity={0.45} /></mesh>
        <mesh position={[0, 1.1, 0.03]}><planeGeometry args={[2.55, 0.64]} /><meshBasicMaterial map={label} transparent opacity={0.85} depthWrite={false} /></mesh>
        {[0, 1, 2, 3, 4, 5].map((line) => <group key={line} position={[0, 0.3 - line * 0.31, 0.04]}>
          <mesh><boxGeometry args={[2.4, 0.008, 0.008]} /><meshBasicMaterial color={MUTED} transparent opacity={0.45} /></mesh>
          <mesh position={[-0.55, 0.12, 0]}><boxGeometry args={[0.85, 0.035, 0.008]} /><meshBasicMaterial color={line % 2 ? CORE : INK} transparent opacity={0.3 + line * 0.05} /></mesh>
        </group>)}
      </group>)}
      <mesh position={[0, 2.25, 0]}><boxGeometry args={[10.2, 0.012, 0.012]} /><meshBasicMaterial color={MUTED} transparent opacity={0.5} /></mesh>
    </group>)}
    <group ref={publicOdds} position={[0, 1.15, -12]} scale={0.6}><OddsBars yes={yes} no={no} step={0} /></group>
    <group ref={payout} position={[0, 0.9, -25]}>{[0, 1, 2].map((i) => <group key={i} position={[(i - 1) * 1.55, 0, 0]}>
      <mesh scale={0.5}><icosahedronGeometry args={[1, 0]} /><meshPhysicalMaterial color={ICE} flatShading roughness={0.12} transmission={0.4} thickness={0.5} clearcoat={1} transparent opacity={0.9} /><Edges color={CORE} transparent opacity={0.8} /></mesh>
      <mesh position={[0, -0.75, 0]}><planeGeometry args={[0.9, 0.22]} /><meshBasicMaterial map={label} transparent /></mesh>
    </group>)}</group>
  </group>;
}

export default function HeroScene({ active = true, onReady, onFailure, ...props }: SceneProps) {
  return <Canvas frameloop={active ? "always" : "never"} dpr={[1, 1.25]} camera={{ position: [0, 0.1, 6.5], fov: 37 }} gl={{ antialias: true, alpha: false, powerPreference: "low-power" }} onCreated={({ gl }) => { gl.setClearColor(PAPER); gl.domElement.addEventListener("webglcontextlost", () => onFailure?.(), { once: true }); onReady?.(); }}>
    <fog attach="fog" args={[PAPER, 9, 34]} />
    <JourneyCamera journey={props.journey} />
    <ambientLight intensity={1.3} />
    <directionalLight position={[-3, 5, 4]} intensity={2.2} />
    <Environment resolution={128} frames={1}>
      <Lightformer form="rect" intensity={4} position={[-3, 2, 3]} scale={[2, 6, 1]} target={[0, 0, 0]} />
      <Lightformer form="rect" intensity={3} position={[3, 1, 2]} scale={[1, 5, 1]} target={[0, 0, 0]} />
      <Lightformer form="rect" intensity={2.5} position={[0, 4, 0]} scale={[6, 1, 1]} target={[0, 0, 0]} />
    </Environment>
    <Silhouettes step={props.step} journey={props.journey} /><Specimen {...props} /><Shards journey={props.journey} /><Snow /><LedgerWorld yes={props.yes} no={props.no} journey={props.journey} />
  </Canvas>;
}

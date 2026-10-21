"use client";

// Procedural frozen-glass prism (docs/DESIGN.md §6). No external model files.
import { MeshTransmissionMaterial } from "@react-three/drei";
import { Canvas, useFrame } from "@react-three/fiber";
import { type ComponentRef, useMemo, useRef } from "react";
import * as THREE from "three";

export type SceneProps = {
  yes: number | null;
  no: number | null;
  /** Scrollytelling step 0..4 (0 = hero). */
  step: number;
};

const YES = new THREE.Color("#7FD1B9");
const NO = new THREE.Color("#E58C8A");
const NEUTRAL = new THREE.Color("#8FA8C8");

/** Pre-blurred capsule texture: silhouettes stay blurred no matter how clear the glass gets. */
function useBlurredCapsule(): THREE.Texture {
  return useMemo(() => {
    const c = document.createElement("canvas");
    c.width = 64;
    c.height = 160;
    const ctx = c.getContext("2d")!;
    ctx.filter = "blur(10px)";
    ctx.fillStyle = "rgba(220,235,255,0.55)";
    ctx.beginPath();
    ctx.roundRect(16, 16, 32, 128, 16);
    ctx.fill();
    const tex = new THREE.CanvasTexture(c);
    tex.colorSpace = THREE.SRGBColorSpace;
    return tex;
  }, []);
}

function Silhouettes({ step }: { step: number }) {
  const tex = useBlurredCapsule();
  const group = useRef<THREE.Group>(null);
  const seeds = useMemo(() => Array.from({ length: 7 }, (_, i) => ({ x: -1.6 + i * 0.55, phase: i * 1.7 })), []);
  useFrame(({ clock }) => {
    const t = clock.getElapsedTime();
    group.current?.children.forEach((child, i) => {
      const s = seeds[i];
      child.position.y = Math.sin(t * 0.3 + s.phase) * 0.35;
      child.position.x = s.x + Math.sin(t * 0.17 + s.phase) * 0.12;
    });
  });
  // Step 3 ("sender stays blurred") brings the silhouettes forward, still blurred.
  return (
    <group ref={group} position={[0, 0, step === 3 ? -1.2 : -2]}>
      {seeds.map((s, i) => (
        <sprite key={i} position={[s.x, 0, 0]} scale={[0.35, 0.9, 1]}>
          <spriteMaterial map={tex} transparent opacity={0.5} depthWrite={false} />
        </sprite>
      ))}
    </group>
  );
}

function OddsBars({ yes, no }: { yes: number | null; no: number | null }) {
  const yesRef = useRef<THREE.Mesh>(null);
  const noRef = useRef<THREE.Mesh>(null);
  const known = yes !== null && no !== null;
  useFrame(() => {
    const ty = known ? Math.max(yes!, 0.02) : 0.5;
    const tn = known ? Math.max(no!, 0.02) : 0.5;
    for (const [ref, target] of [[yesRef, ty], [noRef, tn]] as const) {
      const m = ref.current;
      if (!m) continue;
      m.scale.y = THREE.MathUtils.lerp(m.scale.y, target * 1.8, 0.08);
      m.position.y = -0.9 + m.scale.y / 2;
    }
  });
  return (
    <group>
      <mesh ref={yesRef} position={[-0.28, -0.4, 0]}>
        <boxGeometry args={[0.32, 1, 0.32]} />
        <meshBasicMaterial color={known ? YES : NEUTRAL} />
      </mesh>
      <mesh ref={noRef} position={[0.28, -0.4, 0]}>
        <boxGeometry args={[0.32, 1, 0.32]} />
        <meshBasicMaterial color={known ? NO : NEUTRAL} />
      </mesh>
    </group>
  );
}

function Prism({ step }: { step: number }) {
  const mesh = useRef<THREE.Mesh>(null);
  const material = useRef<ComponentRef<typeof MeshTransmissionMaterial>>(null);
  useFrame(({ pointer }, delta) => {
    if (mesh.current) mesh.current.rotation.y += delta * 0.12;
    // Cursor near the center (where the odds bars are) lowers frost locally; step 2 clears it.
    const near = Math.max(0, 1 - Math.hypot(pointer.x, pointer.y) * 1.6);
    const target = step === 2 ? 0.05 : 0.32 - near * 0.24;
    // The transmission material is a MeshPhysicalMaterial at runtime; roughness is a plain uniform.
    const m = material.current as unknown as { roughness: number } | null;
    if (m) m.roughness = THREE.MathUtils.lerp(m.roughness, target, 0.06);
  });
  return (
    <mesh ref={mesh} scale={step === 4 ? 1.1 : 1}>
      <boxGeometry args={[1.6, 2.2, 1.6]} />
      <MeshTransmissionMaterial
        ref={material}
        samples={4}
        resolution={256}
        transmission={1}
        thickness={0.8}
        roughness={0.32}
        chromaticAberration={0.03}
        backside={false}
        color="#DCEBFF"
      />
    </mesh>
  );
}

export default function HeroScene({ yes, no, step }: SceneProps) {
  return (
    <Canvas dpr={[1, 1.5]} camera={{ position: [0, 0, step >= 2 ? 4.2 : 5], fov: 40 }} gl={{ antialias: true, alpha: true }}>
      <ambientLight intensity={0.6} />
      <directionalLight position={[3, 4, 5]} intensity={1.2} />
      <Silhouettes step={step} />
      <OddsBars yes={yes} no={no} />
      <Prism step={step} />
    </Canvas>
  );
}

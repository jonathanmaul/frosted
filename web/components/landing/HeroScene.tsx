"use client";

// Entire specimen is procedural; material API verified against installed drei (docs/DESIGN.md §6).
import { Edges, Environment, Lightformer, MeshTransmissionMaterial, useGLTF } from "@react-three/drei";
import { Canvas, useFrame } from "@react-three/fiber";
import { useEffect, useLayoutEffect, useMemo, useRef, type ComponentRef, type RefObject } from "react";
import * as THREE from "three";

import { journeyFrame, type JourneyState } from "@/lib/journey";
import { formatPercent } from "@/lib/api";

export type SceneProps = { yes: number | null; no: number | null; step: number; journey: RefObject<JourneyState>; active?: boolean; onReady?: () => void; onFailure?: () => void };

function Silhouettes({ step, journey }: Pick<SceneProps, "step" | "journey">) {
  const group = useRef<THREE.Group>(null);
  const texture = useMemo(() => {
    const c = document.createElement("canvas");
    c.width = 96; c.height = 192;
    const ctx = c.getContext("2d")!;
    // Blur is baked into the texture: cursor clearing can never reveal an identity.
    ctx.filter = "blur(13px)";
    ctx.fillStyle = "#8FA8C8";
    ctx.beginPath(); ctx.roundRect(31, 26, 34, 136, 17); ctx.fill();
    return new THREE.CanvasTexture(c);
  }, []);
  useEffect(() => () => texture.dispose(), [texture]);
  useFrame(({ clock }) => {
    const t = clock.getElapsedTime();
    const p = journey.current.active ? journey.current.progress : 0;
    const spread = THREE.MathUtils.smoothstep(p, 0.16, 0.55);
    if (group.current) { group.current.position.z = -1.2 - spread * 14; group.current.scale.setScalar(1 + spread * 1.8); }
    group.current?.children.forEach((child, i) => {
      child.position.y = Math.sin(t * 0.19 + i * 1.7) * 0.3 + (i % 2 ? 0.55 : -0.35);
      child.position.x = -1.8 + i * 0.6 + Math.sin(t * 0.12 + i) * 0.1;
    });
  });
  return <group ref={group} position={[0, 0, -1.2]}>{Array.from({ length: 7 }, (_, i) => <sprite key={i} scale={[0.55, 1.1, 1]}><spriteMaterial map={texture} transparent opacity={step === 3 ? 0.55 : 0.28} depthWrite={false} /></sprite>)}</group>;
}

function OddsLabel({ yes, no }: Pick<SceneProps, "yes" | "no">) {
  const texture = useMemo(() => {
    const c = document.createElement("canvas"); c.width = 768; c.height = 256;
    const ctx = c.getContext("2d")!;
    ctx.font = '400 46px "IBM Plex Mono"'; ctx.textAlign = "center";
    ctx.fillStyle = yes === null ? "#8FA8C8" : "#7FD1B9"; ctx.fillText(`YES  ${formatPercent(yes)}`, 196, 120);
    ctx.fillStyle = no === null ? "#8FA8C8" : "#E58C8A"; ctx.fillText(`NO  ${formatPercent(no)}`, 572, 120);
    const tex = new THREE.CanvasTexture(c); tex.colorSpace = THREE.SRGBColorSpace;
    return tex;
  }, [yes, no]);
  useEffect(() => () => texture.dispose(), [texture]);
  return <mesh position={[0, -0.94, 0.17]}><planeGeometry args={[2, 0.67]} /><meshBasicMaterial map={texture} transparent depthWrite={false} /></mesh>;
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
  return <><group ref={bars}>{["#7FD1B9", "#E58C8A"].map((color, i) => <mesh key={color} position={[i === 0 ? -0.42 : 0.42, 0, 0]}><boxGeometry args={[0.48, 1, 0.36]} /><meshPhysicalMaterial color={known ? color : "#8FA8C8"} roughness={0.17} metalness={0.35} clearcoat={1} transmission={0.22} thickness={0.3} transparent opacity={known ? 0.85 : 0.28} /><Edges color={known ? color : "#8FA8C8"} transparent opacity={0.5} /></mesh>)}</group><OddsLabel yes={yes} no={no} /></>;
}

function Specimen({ yes, no, step, journey }: SceneProps) {
  const group = useRef<THREE.Group>(null);
  const hovered = useRef(false);
  const crystal = useGLTF("/models/frost-ledger.glb");
  const shell = crystal.nodes.IceShell as THREE.Mesh;
  const inclusions = useMemo(() => {
    const scene = crystal.scene.clone(true);
    const outer = scene.getObjectByName("IceShell");
    if (outer) outer.visible = false;
    return scene;
  }, [crystal.scene]);
  const frost = useMemo(() => {
    const pixels = new Uint8Array(128 * 128 * 4);
    let seed = 947;
    for (let i = 0; i < pixels.length; i += 4) {
      seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0;
      const tone = 160 + (seed % 96);
      pixels[i] = pixels[i + 1] = pixels[i + 2] = tone; pixels[i + 3] = 255;
    }
    const texture = new THREE.DataTexture(pixels, 128, 128);
    texture.wrapS = texture.wrapT = THREE.RepeatWrapping;
    texture.repeat.set(3, 3); texture.needsUpdate = true;
    return texture;
  }, []);
  useEffect(() => () => frost.dispose(), [frost]);
  const material = useRef<ComponentRef<typeof MeshTransmissionMaterial>>(null);
  const uniforms = useMemo(() => ({ frostCursor: { value: new THREE.Vector2(-2, -2) }, frostSize: { value: new THREE.Vector2(1, 1) } }), []);
  useLayoutEffect(() => {
    // Drei types its ref as JSX props; the runtime instance is a physical material.
    const m = material.current as unknown as THREE.MeshPhysicalMaterial | null;
    if (!m) return;
    const original = m.onBeforeCompile;
    m.onBeforeCompile = (shader, renderer) => {
      original.call(m, shader, renderer);
      Object.assign(shader.uniforms, uniforms);
      shader.fragmentShader = `uniform vec2 frostCursor; uniform vec2 frostSize;\n${shader.fragmentShader}`;
      shader.fragmentShader = shader.fragmentShader.replace("#include <roughnessmap_fragment>", `#include <roughnessmap_fragment>
        vec2 frostDelta = (gl_FragCoord.xy / frostSize - frostCursor) * vec2(frostSize.x / frostSize.y, 1.0);
        float clearPatch = 1.0 - smoothstep(0.025, 0.15, length(frostDelta));
        roughnessFactor = mix(roughnessFactor, 0.045, clearPatch);
      `);
    };
    m.needsUpdate = true;
    return () => { m.onBeforeCompile = original; };
  }, [uniforms]);
  useFrame(({ clock, pointer, gl }, delta) => {
    const t = clock.getElapsedTime();
    gl.getDrawingBufferSize(uniforms.frostSize.value);
    uniforms.frostCursor.value.set(hovered.current ? (pointer.x + 1) / 2 : -2, hovered.current ? (pointer.y + 1) / 2 : -2);
    if (group.current) {
      const alignment = journey.current.active ? THREE.MathUtils.smoothstep(journey.current.progress, 0.04, 0.17) : 0;
      group.current.rotation.y = THREE.MathUtils.damp(group.current.rotation.y, (-0.44 + Math.sin(t * 0.13) * 0.16) * (1 - alignment), 3, delta);
      group.current.rotation.z = THREE.MathUtils.damp(group.current.rotation.z, -0.1 * (1 - alignment), 3, delta);
      group.current.position.y = Math.sin(t * 0.22) * 0.045 * (1 - alignment);
    }
  });
  return <group ref={group} onPointerOver={() => { hovered.current = true; }} onPointerOut={() => { hovered.current = false; }} rotation={[0.12, -0.44, -0.1]}>
    <OddsBars yes={yes} no={no} step={step} />
    <primitive object={inclusions} dispose={null} />
    <mesh geometry={shell.geometry}>
      <MeshTransmissionMaterial ref={material} samples={5} resolution={384} transmission={1} thickness={1.15} roughness={0.28} roughnessMap={frost} chromaticAberration={0.008} anisotropicBlur={0.08} distortion={0.16} distortionScale={0.18} temporalDistortion={0} ior={1.31} clearcoat={1} clearcoatRoughness={0.07} color="#DCEBFF" envMapIntensity={1.5} background={new THREE.Color("#070B12")} />
    </mesh>
  </group>;
}

/** The camera traverses the original specimen; progress never triggers a React render per frame. */
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
  const crystal = useGLTF("/models/frost-ledger.glb");
  const cutGlass = (crystal.nodes.IceShell as THREE.Mesh).geometry;
  const label = useMemo(() => {
    const canvas = document.createElement("canvas"); canvas.width = 1024; canvas.height = 256;
    const ctx = canvas.getContext("2d")!;
    ctx.fillStyle = "#8FA8C8"; ctx.font = '400 28px "IBM Plex Mono"';
    ctx.fillText("GLASS ESCROW / PUBLIC VIEWING KEY", 36, 64);
    ctx.fillStyle = "#DCEBFF"; ctx.font = '400 40px "IBM Plex Mono"';
    ctx.fillText("AMOUNT     OUTCOME     BLOCK", 36, 144);
    ctx.fillStyle = "#8FA8C8"; ctx.fillText("SENDER     ░▒▓░▒▓░▒▓░▒▓░▒▓", 36, 216);
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
    <gridHelper args={[70, 35, "#8FA8C8", "#263346"]} position={[0, -2.1, -18]} material-transparent material-opacity={0.18} />
    {[0, 1, 2, 3].map((row) => <group key={row} position={[0, 0, -4 - row * 5.5]}>
      {[-1, 1].map((side) => <group key={side} position={[side * 3.7, 0.2, 0]} rotation={[0, side * -0.3, side * 0.035]}>
        <mesh><boxGeometry args={[2.8, 3.9, 0.035]} /><meshPhysicalMaterial color="#8FA8C8" metalness={0.2} roughness={0.28} transparent opacity={0.055} /><Edges color="#8FA8C8" transparent opacity={0.18} /></mesh>
        <mesh position={[0, 1.1, 0.03]}><planeGeometry args={[2.55, 0.64]} /><meshBasicMaterial map={label} transparent opacity={0.4} depthWrite={false} /></mesh>
        {[0, 1, 2, 3, 4, 5].map((line) => <group key={line} position={[0, 0.3 - line * 0.31, 0.04]}>
          <mesh><boxGeometry args={[2.4, 0.008, 0.008]} /><meshBasicMaterial color="#8FA8C8" transparent opacity={0.25} /></mesh>
          <mesh position={[-0.55, 0.12, 0]}><boxGeometry args={[0.85, 0.035, 0.008]} /><meshBasicMaterial color="#DCEBFF" transparent opacity={0.13 + line * 0.025} /></mesh>
        </group>)}
      </group>)}
      <mesh position={[0, 2.25, 0]}><boxGeometry args={[10.2, 0.012, 0.012]} /><meshBasicMaterial color="#8FA8C8" transparent opacity={0.22} /></mesh>
    </group>)}
    <group ref={publicOdds} position={[0, 0.55, -12]} scale={0.9}><OddsBars yes={yes} no={no} step={0} /></group>
    <group ref={payout} position={[0, 0.3, -25]}>{[0, 1, 2].map((i) => <group key={i} position={[(i - 1) * 1.55, 0, 0]}>
      <mesh geometry={cutGlass} scale={[0.36, 0.4, 0.2]}><meshPhysicalMaterial color="#DCEBFF" metalness={0.15} roughness={0.12} transmission={0.5} thickness={0.16} clearcoat={1} transparent opacity={0.55} /><Edges color="#DCEBFF" transparent opacity={0.45} /></mesh>
      <mesh position={[0, 0.05, 0.07]}><planeGeometry args={[0.52, 0.13]} /><meshBasicMaterial map={label} transparent /></mesh>
    </group>)}</group>
  </group>;
}

export default function HeroScene({ active = true, onReady, onFailure, ...props }: SceneProps) {
  return <Canvas frameloop={active ? "always" : "never"} dpr={[1, 1.25]} camera={{ position: [0, 0.1, 6.5], fov: 37 }} gl={{ antialias: true, alpha: false, powerPreference: "low-power" }} onCreated={({ gl }) => { gl.setClearColor("#070B12"); gl.domElement.addEventListener("webglcontextlost", () => onFailure?.(), { once: true }); onReady?.(); }}>
    <fog attach="fog" args={["#070B12", 8, 32]} />
    <JourneyCamera journey={props.journey} />
    <ambientLight intensity={0.6} />
    <directionalLight position={[-3, 5, 4]} intensity={2.8} />
    <Environment resolution={128} frames={1}>
      <Lightformer form="rect" intensity={5} position={[-3, 2, 3]} scale={[1, 7, 1]} target={[0, 0, 0]} />
      <Lightformer form="rect" intensity={4} position={[3, 1, 2]} scale={[0.35, 5, 1]} target={[0, 0, 0]} />
      <Lightformer form="rect" intensity={2} position={[1, -2, -3]} scale={[3, 0.3, 1]} target={[0, 0, 0]} />
      <Lightformer form="rect" intensity={1.2} position={[0, 4, 0]} scale={[5, 1, 1]} target={[0, 0, 0]} />
    </Environment>
    <Silhouettes step={props.step} journey={props.journey} /><Specimen {...props} /><LedgerWorld yes={props.yes} no={props.no} journey={props.journey} />
  </Canvas>;
}

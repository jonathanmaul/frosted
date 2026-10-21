import nextVitals from "eslint-config-next/core-web-vitals";
import nextTs from "eslint-config-next/typescript";

const config = [
  ...nextVitals,
  ...nextTs,
  // components/bits/ is vendored React Bits source (see its README); kept close to upstream, not linted.
  { ignores: [".next/**", "node_modules/**", "next-env.d.ts", "components/bits/**"] },
];

export default config;

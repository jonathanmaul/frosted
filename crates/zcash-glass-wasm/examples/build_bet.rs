//! Local fixture bridge for the regtest wallet harness; prints secrets to stdout only.
use zcash_glass::envelope::SealRandomness;
fn main() {
    let path = std::env::args().nth(1).expect("input JSON path");
    let input = serde_json::from_str(&std::fs::read_to_string(path).expect("read input"))
        .expect("valid BuildBet JSON");
    let output = zcash_glass_wasm::build_with_randomness(
        input,
        SealRandomness {
            env_esk: [0x22; 32],
            env_nonce: [0x33; 24],
        },
        [0x55; 8],
    )
    .expect("build fixture bet");
    println!("{}", serde_json::to_string(&output).unwrap());
}

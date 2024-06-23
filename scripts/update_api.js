const { execSync } = require('child_process')
const { mkdirSync, rmSync, cpSync } = require('fs')
const path = require('path')
const { ROOT, CLIENT_ROOT } = require('./base')
const { generateApiRs } = require('./generate_api_rs')

generateApiRs()

// console.log("[Script] Start codegen")
// execSync("flutter_rust_bridge_codegen --rust-input ./rust-libs/ease-client/src/api.rs --dart-output ./lib/bridge_generated.dart", {
//     cwd: ROOT,
//     stdio: 'inherit'
// })

// console.log("[Script] Start build client")
// execSync("cargo build", {
//     cwd: CLIENT_ROOT,
//     stdio: 'inherit'
// })

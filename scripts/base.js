const { execSync } = require('child_process')
const { mkdirSync, rmSync, cpSync } = require('fs')
const path = require('path')

const ROOT = path.resolve(__dirname, '../')
const RUST_LIBS_ROOTS = path.resolve(ROOT, "./rust-libs")
const CLIENT_ROOT = path.resolve(ROOT, "./rust-libs/ease-client")

module.exports.ROOT = ROOT
module.exports.RUST_LIBS_ROOTS = RUST_LIBS_ROOTS
module.exports.CLIENT_ROOT = CLIENT_ROOT
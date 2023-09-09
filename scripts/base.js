const { execSync } = require('child_process')
const { mkdirSync, rmSync, cpSync } = require('fs')
const path = require('path')

const ROOT = path.resolve(__dirname, '../')
const CLIENT_ROOT = path.resolve(ROOT, "./rust-libs/ease-client")

module.exports.ROOT = ROOT
module.exports.CLIENT_ROOT = CLIENT_ROOT
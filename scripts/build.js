const { execSync } = require('child_process')
const { mkdirSync, rmSync, cpSync } = require('fs')
const { ROOT, CLIENT_ROOT } = require('./base')
const path = require('path')

execSync('flutter build apk --release --split-per-abi', { stdio: 'inherit', cwd: ROOT })

const srcDir = path.resolve(ROOT, './build/app/outputs/flutter-apk')
const dstDir = path.resolve(ROOT, './artifacts/apk')

rmSync(dstDir, { recursive: true, force: true })
mkdirSync(srcDir, { recursive: true })
cpSync(srcDir, dstDir, { recursive: true })

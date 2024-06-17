const { execSync } = require('child_process')
const { mkdirSync, rmSync, cpSync, writeFileSync } = require('fs')
const { ROOT, CLIENT_ROOT } = require('./base')
const path = require('path')

const androidDir = path.resolve(ROOT, './android')
const jksPath = path.resolve(androidDir, 'ease_music_player.jks')
const keyPropertiesPath = path.resolve(androidDir, 'key.properties')
const srcDir = path.resolve(ROOT, './build/app/outputs/flutter-apk')
const dstDir = path.resolve(ROOT, './artifacts/apk')

// Signing
writeFileSync(keyPropertiesPath, `storePassword=${process.env.ANDROID_SIGN_PASSWORD}
keyPassword=${process.env.ANDROID_SIGN_PASSWORD}
keyAlias=key0
storeFile=ease_music_player.jks`)
console.log(`${keyPropertiesPath} written`)

const jks = Buffer.from(process.env.ANDROID_SIGN_JKS, 'base64')
writeFileSync(jksPath, jks);
console.log(`${jksPath} written`)

execSync('flutter build apk --release --split-per-abi', { stdio: 'inherit', cwd: ROOT })

rmSync(dstDir, { recursive: true, force: true })
mkdirSync(srcDir, { recursive: true })
cpSync(srcDir, dstDir, { recursive: true })

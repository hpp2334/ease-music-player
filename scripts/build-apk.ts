import { execSync } from "child_process"
import { writeFileSync, rmSync, mkdirSync, cpSync } from "fs"
import path from "path"
import { ROOT } from "./base"


const androidDir = path.resolve(ROOT, './android')
const jksPath = path.resolve(androidDir, 'root.jks')
const keyPropertiesPath = path.resolve(androidDir, 'key.properties')
const srcDir = path.resolve(androidDir, './app/build/outputs/apk/release')
const dstDir = path.resolve(ROOT, './artifacts/apk')

// Signing
writeFileSync(keyPropertiesPath, `storePassword=${process.env.ANDROID_SIGN_PASSWORD}
    keyPassword=${process.env.ANDROID_SIGN_PASSWORD}
    keyAlias=key0
    storeFile=root.jks`)
console.log(`${keyPropertiesPath} written`)

execSync("./gradlew assembleRelease", {
    stdio: 'inherit',
    cwd: androidDir
})

rmSync(dstDir, { recursive: true, force: true })
mkdirSync(srcDir, { recursive: true })
console.log(srcDir)
cpSync(srcDir, dstDir, { recursive: true })

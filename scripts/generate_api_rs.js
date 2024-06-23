
const { execSync } = require('child_process')
const { mkdirSync, rmSync, cpSync, readdirSync, existsSync, readFileSync, writeFileSync } = require('fs')
const path = require('path')
const { ROOT, CLIENT_ROOT } = require('./base')

const log = (...msg) => {
    console.log('[Generate api.rs]', ...msg)
}


const modulesDir = path.resolve(CLIENT_ROOT, './src/modules')
const apiRsPath = path.resolve(CLIENT_ROOT, './src/api.rs')
const moduleNames = readdirSync(modulesDir)

// use text matching way to deal with most cases
function parseCode(code) {
    const marker = '__$MARKER$__'
    /** @type {Array<string>}  */
    const tokens = code.replaceAll(/\s+/g, marker).replaceAll(/([\(\),:{}?;])/g, `${marker}$1${marker}`).split(marker).filter(Boolean)

    const ret = []

    let i = 0
    while (i + 10 < tokens.length) {
        if (tokens[i] === 'pub'
            && tokens[i + 1] === 'fn'
            && tokens[i + 2].startsWith('controller_')
            && tokens[i + 3] === '('
            && tokens[i + 4] === 'ctx'
            && tokens[i + 5] === ':'
            && tokens[i + 6] === 'MistyControllerContext'
            && tokens[i + 7] === ',') {

            const controllerName = tokens[i + 2]
            const fnName = controllerName.slice('controller_'.length)
            let argTypeName = tokens[i + 10]
            if (argTypeName === '(') {
                argTypeName = '()'
            }

            ret.push({
                fnName,
                controllerName,
                argTypeName,
            })
            i += 10
        } else {
            i += 1
        }
    }
    return ret
}

module.exports.generateApiRs = function () {
    let apiRsContent = readFileSync(apiRsPath, 'utf-8')
    log('api.rs read')

    const APIRS_MARKER = '// API_GENERATE_MARKER'
    const apiRsMarkerIndex = apiRsContent.indexOf(APIRS_MARKER)
    if (apiRsMarkerIndex >= 0) {
        apiRsContent = apiRsContent.slice(0, apiRsMarkerIndex)
        apiRsContent += APIRS_MARKER + '\n'
    } else {
        apiRsContent += '\n' + APIRS_MARKER + '\n'
    }

    log("Scan controllers")
    for (const moduleName of moduleNames) {
        const controllerFilePath = path.resolve(modulesDir, moduleName, './controller.rs')
        if (!existsSync(controllerFilePath)) {
            continue
        }
        log(`parse ${moduleName} controller`)
        const code = readFileSync(controllerFilePath, 'utf-8')
        const list = parseCode(code)

        for (const { fnName, controllerName, argTypeName } of list) {
            apiRsContent += `#[uniffi::export]\n`
            if (argTypeName !== '()') {
                apiRsContent += `pub fn ${fnName}(arg: ${argTypeName}) -> ApiRet {\n`
                apiRsContent += `    let ret = call_controller(${controllerName}, arg)?;\n`
            } else {
                apiRsContent += `pub fn ${fnName}() -> ApiRet {\n`
                apiRsContent += `    let ret = call_controller(${controllerName}, ())?;\n`
            }

            apiRsContent += `    Ok(ret)\n`
            apiRsContent += `}\n\n`
        }
    }

    writeFileSync(apiRsPath, apiRsContent)
    log('api.rs written')
}

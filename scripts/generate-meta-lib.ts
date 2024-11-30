import * as fs from "fs";
import * as path from "path";
import { RUST_LIBS_ROOTS } from "./base";

interface Meta {
    module: string;
    funcName: string;
    argument: string;
    return: string;
    code: string;
}

function parseRustCode(code: string, module: string): Meta[] {
    const PREFIX_PUB_CRATE = "pub(crate) async fn c"
    const PREFIX_PUB = "pub async fn c"

    const isLineMatch = (s: string) => s.startsWith(PREFIX_PUB_CRATE) || s.startsWith(PREFIX_PUB)

    const lines = code.split("\n");
    const metas: Meta[] = [];
    let currentLine = "";

    function flush() {
        if (!isLineMatch(currentLine)) {
            currentLine = "";
            return
        }

        const tokens = tokenize(currentLine);
        const meta = parseTokens(tokens, module);
        if (meta) {
            metas.push(meta);
        }
        currentLine = "";
    }

    for (const line of lines) {
        if (isLineMatch(line)) {
            flush()
        }

        if (currentLine) {
            currentLine += " " + line;
        } else {
            currentLine = line
        }
    }

    flush();

    return metas;
}

function tokenize(line: string): string[] {
    const rawTokens: string[] = [];

    {
        const SEPS = [
            "(",
            ")",
            "<",
            ">",
            "[",
            "]",
            "&",
            "-",
            ":",
            ",",
            "*",
        ]
        let currentToken = "";

        const pushCurrent = () => {
            if (currentToken) {
                rawTokens.push(currentToken);
                currentToken = "";
            }
        }

        for (const char of line) {
            if (char === " " || char === "\n") {
                pushCurrent()
            } else if (SEPS.includes(char)) {
                pushCurrent()
                rawTokens.push(char);
            } else {
                currentToken += char;
            }
        }

        if (currentToken) {
            rawTokens.push(currentToken);
        }
    }

    return rawTokens;
}

function parseTokens(tokens: string[], module: string): Meta | null {
    let cursor = 0

    const moveTo = (s: string) => {
        let acc = ''

        while (cursor < tokens.length) {
            const token = tokens[cursor]
            if (token === s) {
                return acc
            }
            cursor += 1
            acc += token
        }
        throw Error(`cannot moveTo ${s}`)
    }

    moveTo('fn')
    const funcName = moveTo('(').slice(2);
    moveTo(':')
    cursor += 1
    moveTo(':')
    const argument = moveTo('-').slice(1, -1).replace(/[,]+$/, '');
    moveTo('<')
    cursor += 1
    const returnType = moveTo('{').slice(0, -1);

    const funcNameCamel = upperFirst(toCamelCase(removePrefix(funcName)));

    return {
        module,
        funcName: funcName,
        argument,
        return: returnType,
        code: funcNameCamel,
    };
}

function removePrefix(funcName: string): string {
    const firstUnderscoreIndex = funcName.indexOf("_");
    if (firstUnderscoreIndex > 0) {
        return funcName.slice(firstUnderscoreIndex + 1);
    }
    return funcName;
}

function toCamelCase(str: string): string {
    return str
        .split("_")
        .map((word, index) =>
            index === 0 ? word : word.charAt(0).toUpperCase() + word.slice(1),
        )
        .join("");
}

function upperFirst(str: string): string {
    return str.charAt(0).toUpperCase() + str.slice(1);
}

function processDirectory(directory: string): Meta[] {
    const metas: Meta[] = [];

    const files = fs.readdirSync(directory);
    for (const file of files) {
        const filePath = path.join(directory, file);
        if (path.extname(filePath) === ".rs") {
            const module = path.basename(filePath, ".rs");
            const code = fs.readFileSync(filePath, "utf-8");
            const fileMetas = parseRustCode(code, module);
            metas.push(...fileMetas);
        }
    }

    return metas;
}

export function generateMetas() {
    const controllersDir = path.resolve(RUST_LIBS_ROOTS, "ease-client-backend/src/controllers")
    const controllerGenerated = path.resolve(controllersDir, "generated.rs")
    const codeGenerated = path.resolve(RUST_LIBS_ROOTS, "ease-client-shared/src/backends/generated.rs")
    const metas = processDirectory(controllersDir)

    const allModules = [...new Set(metas.map(v => v.module))]

    // codes
    {
        let s = ''
        s += `#![allow(unused_imports)]\n`
        for (const module of allModules) {
            s += `use crate::backends::${module}::*;\n`
        }
        s += '\n'
        s += `#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq, Eq, Clone, Copy)]\n`
        s += `pub enum Code {\n`
        for (const meta of metas) {
            s += `    ${meta.code},\n`
        }
        s += `}`
        s += '\n'
        for (const meta of metas) {
            s += `define_message! {\n`
            s += `    ${meta.code}Msg,\n`
            s += `    Code::${meta.code},\n`
            s += `    ${meta.argument},\n`
            s += `    ${meta.return}\n`
            s += `}\n`
        }
        fs.writeFileSync(codeGenerated, s)
    }
    // controllers
    {
        let s = ''
        s += `#![allow(unused_imports)]\n`
        s += `generate_dispatch_message! {\n`
        for (let i = 0; i < metas.length; i++) {
            const meta = metas[i]
            const isLast = i === metas.length - 1
            s += `    ease_client_shared::backends::generated::${meta.code}Msg,\n`
            s += `    super::${meta.module}::${meta.funcName}${isLast ? '' : ','}\n`
        }
        s += `}`
        fs.writeFileSync(controllerGenerated, s)
    }
}

import path from 'path'

export const ROOT = path.resolve(__dirname, '../')
export const RUST_LIBS_ROOTS = path.resolve(ROOT, "./rust-libs")
export const CLIENT_ROOT = path.resolve(ROOT, "./rust-libs/ease-client")
export const ENVS = {
    Build: Boolean(process.env.EBUILD)
}
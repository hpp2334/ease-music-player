/** @type {import('@rspack/core').Configuration} */
module.exports = {
    mode: "production",
    target: ["web", "es2020"],
    entry: {
        backend: "./src/backend.ts",
        view: "./src/view.ts",
    },
    output: {
        path: __dirname + "/dist",
        filename: "[name].js",
        library: { type: "module" },
        clean: true,
    },
    experiments: {
        outputModule: true,
    },
    resolve: {
        extensions: [".ts", ".js"],
    },
    externalsType: "module",
    externals: [
        /^tur:/,
        "ease",
    ],
    optimization: {
        minimize: false,
    },
    module: {
        rules: [
            {
                test: /\.ts$/,
                exclude: /node_modules/,
                use: [
                    {
                        loader: "builtin:swc-loader",
                        options: {
                            jsc: {
                                parser: { syntax: "typescript" },
                                target: "es2020",
                            },
                        },
                    },
                ],
            },
        ],
    },
};

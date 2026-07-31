/** @type {import('@rspack/core').Configuration} */
module.exports = {
    mode: "production",
    target: ["web", "es2020"],
    entry: "./src/index.ts",
    output: {
        path: __dirname + "/../../android/app/src/main/assets/plugins/com.ease.turtest",
        filename: "plugin.js",
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
        /^ease:/,
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

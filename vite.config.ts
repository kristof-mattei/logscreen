import path from "node:path";

import { codecovVitePlugin } from "@codecov/vite-plugin";
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import type { UserConfig } from "vite";
import { loadEnv } from "vite";
import { checker } from "vite-plugin-checker";
import svgr from "vite-plugin-svgr";
import viteTsConfigPaths from "vite-tsconfig-paths";
import { coverageConfigDefaults, defineConfig } from "vitest/config";

// https://vitejs.dev/config/
export default defineConfig(({ mode }) => {
    const environment = loadEnv(mode, process.cwd());
    const port = Number.parseInt(environment["VITE_PORT"] ?? "");

    const config: UserConfig = {
        appType: "spa",

        build: {
            emptyOutDir: true,
            outDir: "../../dist",
            rollupOptions: {
                output: {},
            },
            sourcemap: true,
        },
        resolve: {
            alias: {
                "@/": path.resolve("src/"),
                "~bootstrap": path.resolve(import.meta.dirname, "node_modules/bootstrap"),
            },
        },

        plugins: [
            svgr(),
            react(),
            viteTsConfigPaths(),
            checker({ typescript: true }),
            tailwindcss(),
            codecovVitePlugin({
                enableBundleAnalysis: environment["CODECOV_TOKEN"] !== undefined,
                bundleName: "logscreen-front-end",
                uploadToken: environment["CODECOV_TOKEN"] ?? "",
            }),
        ],
        optimizeDeps: {
            exclude: ["src/entrypoints/index.ts"],
        },
        root: "front-end/src",

        server: {
            port: Number.isNaN(port) ? 4000 : port,
            host: true,
            strictPort: true,
            hmr: {
                host: "localhost",
                port: 4000,
            },
            cors: true,
            proxy: {
                "/api": {
                    target: "http://localhost:3000",
                    changeOrigin: true,
                    secure: false,
                    ws: true,
                },
                "/socket.io": {
                    target: "http://localhost:3000",
                    changeOrigin: true,
                    secure: false,
                    ws: true,
                },
            },
        },
        test: {
            coverage: {
                exclude: [...coverageConfigDefaults.exclude, "./dependency-cruiser.config.mjs"],
                reporter: ["json", "html", "text"],
                provider: "v8",
                reportsDirectory: "../../coverage/vitest",
            },
            // environment: "jsdom",
            environmentOptions: {
                // jsdom: {},
            },
            globals: false,
            outputFile: {
                junit: "../../reports/vitest/test-report.xml",
            },
            setupFiles: ["./test.setup.ts"],
        },
    };

    return config;
});

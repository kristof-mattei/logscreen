import nodePath from "node:path";

import { codecovVitePlugin } from "@codecov/vite-plugin";
import babel from "@rolldown/plugin-babel";
import tailwindcss from "@tailwindcss/vite";
import react, { reactCompilerPreset } from "@vitejs/plugin-react";
import type { UserConfig } from "vite";
import { loadEnv } from "vite";
import { checker } from "vite-plugin-checker";
import svgr from "vite-plugin-svgr";
import { coverageConfigDefaults, defineConfig } from "vitest/config";

export default defineConfig(({ mode }) => {
    const environment = loadEnv(mode, process.cwd(), "");
    const port = Number.parseInt(environment["VITE_PORT"] ?? "");

    const config: UserConfig = {
        appType: "spa",
        cacheDir: "../../node_modules/.cache/",
        build: {
            minify: false,
            target: "esnext",
            emptyOutDir: true,
            sourcemap: true,
            outDir: "../../dist",
            rollupOptions: {
                output: {},
            },
        },
        resolve: {
            alias: {
                "@/": nodePath.resolve("src/"),
            },
            tsconfigPaths: true,
        },
        plugins: [
            svgr(),
            react(),
            babel({
                presets: [reactCompilerPreset()],
            }),
            checker({ typescript: true }),
            tailwindcss(),
            codecovVitePlugin({
                enableBundleAnalysis: environment["GITHUB_ACTIONS"] === "true",
                bundleName: "logscreen-front-end",
                oidc: {
                    useGitHubOIDC: true,
                },
                telemetry: false,
            }),
        ],
        optimizeDeps: {
            noDiscovery: true,
            // exclude: ["src/entrypoints/index.ts"],
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
            restoreMocks: true,
            setupFiles: ["./test.setup.ts"],
        },
    };

    return config;
});

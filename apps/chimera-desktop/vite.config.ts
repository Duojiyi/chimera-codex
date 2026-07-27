import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "path";
import packageJson from "./package.json";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: { alias: { "@": path.resolve(__dirname, "./src") } },
  define: { "import.meta.env.VITE_APP_VERSION": JSON.stringify(packageJson.version) },
  server: { host: "127.0.0.1", port: 1421, strictPort: true },
  build: { outDir: "dist", sourcemap: false },
});

/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  darkMode: "class",
  theme: {
    extend: {
      fontFamily: {
        display: ['"Bricolage Grotesque"', "ui-sans-serif", "system-ui", "sans-serif"],
        mono: ['"JetBrains Mono"', "ui-monospace", "SFMono-Regular", "monospace"],
      },
      colors: {
        // copper-amber primary accent
        signal: {
          50: "#fff7ed",
          400: "#fb923c",
          500: "#f97316",
          600: "#ea580c",
          700: "#c2410c",
        },
      },
      boxShadow: {
        panel: "0 1px 2px rgba(15,23,42,0.06), 0 8px 24px -12px rgba(15,23,42,0.18)",
        "panel-dark": "0 1px 0 rgba(255,255,255,0.03) inset, 0 12px 32px -16px rgba(0,0,0,0.7)",
        glow: "0 0 0 3px rgba(249,115,22,0.18)",
      },
      keyframes: {
        pulseDot: {
          "0%,100%": { boxShadow: "0 0 0 0 var(--dot)" },
          "70%": { boxShadow: "0 0 0 6px transparent" },
        },
        riseIn: {
          "0%": { opacity: "0", transform: "translateY(8px)" },
          "100%": { opacity: "1", transform: "translateY(0)" },
        },
        toastIn: {
          "0%": { opacity: "0", transform: "translateY(10px) scale(0.98)" },
          "100%": { opacity: "1", transform: "translateY(0) scale(1)" },
        },
      },
      animation: {
        pulseDot: "pulseDot 2.2s ease-out infinite",
        riseIn: "riseIn 0.4s cubic-bezier(0.16,1,0.3,1) both",
        toastIn: "toastIn 0.32s cubic-bezier(0.16,1,0.3,1) both",
      },
    },
  },
  plugins: [],
};

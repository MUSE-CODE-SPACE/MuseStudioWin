/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        bg: "#0b0d10",
        panel: "#13161b",
        border: "#262a31",
        muted: "#8a93a3",
        fg: "#e7ebf0",
        accent: "#7aa2f7",
      },
    },
  },
  plugins: [],
};

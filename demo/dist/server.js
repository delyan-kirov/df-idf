"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
  return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
const express_1 = __importDefault(require("express"));
const path_1 = __importDefault(require("path"));
const app = (0, express_1.default)();
const PORT = 3000;
// Serve static files from the 'public' folder
app.use(express_1.default.static(path_1.default.join(__dirname, "public")));
app.get("/", (_req, res) => {
  // Assuming some condition here to determine which file to serve
  const condition = false; // Example condition, you should replace this with your own logic
  if (condition) {
    // Serve one HTML file
    res.sendFile(path_1.default.join(__dirname, "public", "index.html"));
  } else {
    // Serve another HTML file
    res.sendFile(path_1.default.join(__dirname, "public", "example.html"));
  }
});
app.listen(PORT, () => {
  console.log(`Server is running on http://localhost:${PORT}`);
});

import express, { Request, Response } from "express";
import path from "path";

const app = express();
const PORT = 3000;

// Serve static files from the 'public' folder
app.use(express.static(path.join(__dirname, "public")));

app.get("/", (_req: Request, res: Response) => {
  // Assuming some condition here to determine which file to serve
  const condition = false; // Example condition, you should replace this with your own logic

  if (condition) {
    // Serve one HTML file
    res.sendFile(path.join(__dirname, "public", "index.html"));
  } else {
    // Serve another HTML file
    res.sendFile(path.join(__dirname, "../dist/public", "example.html"));
  }
});

app.listen(PORT, () => {
  console.log(`Server is running on http://localhost:${PORT}`);
});

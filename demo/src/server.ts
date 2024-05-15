import express, { Request, Response } from "express";
import { exec } from "child_process";
import path from "path";

const app = express();
const PORT = 3000;

// Serve static files from the 'public' folder
app.use(express.static(path.join(__dirname, "../dist/public")));

app.get("/", (_req: Request, res: Response) => {
  res.sendFile(path.join(__dirname, "../dist/public", "example.html"));
});

app.get("/query", (req, res) => {
  const query = req.query.query; // Retrieve the value of the 'query' parameter
  // Process the query here
  console.log("Received query:", query);
  let output: string = "";
  let command: string = `
    cd ../
    ./target/debug/tf_idf -q ${query}`;
  console.log("Cammand:", command);

  // Execute the command
  const childProcess = exec(command, (error, stdout, stderr) => {
    if (error) {
      console.error(`Error: ${error.message}`);
      return;
    }
    if (stderr) {
      console.error(`stderr: ${stderr}`);
      return;
    }
    console.log(stdout);
    output = stdout; // Store the output in the variable
    // Respond with data or perform other actions
    res.send(`Received query: ${output}`);
  });
  // proc the process
  childProcess.on("exit", () => {
    console.log("Output:", output); // Output the stored output
  });
});

app.listen(PORT, () => {
  console.log(`Server is running on http://localhost:${PORT}`);
});

"use strict";
var __importDefault = (this && this.__importDefault) || function (mod) {
    return (mod && mod.__esModule) ? mod : { "default": mod };
};
Object.defineProperty(exports, "__esModule", { value: true });
const express_1 = __importDefault(require("express"));
const child_process_1 = require("child_process");
const path_1 = __importDefault(require("path"));
const app = (0, express_1.default)();
const PORT = 3000;
// Serve static files from the 'public' folder
app.use(express_1.default.static(path_1.default.join(__dirname, "../dist/public")));
app.get("/", (_req, res) => {
    res.sendFile(path_1.default.join(__dirname, "../dist/public", "example.html"));
});
app.get("/query", (req, res) => {
    const query = req.query.query; // Retrieve the value of the 'query' parameter
    // Process the query here
    console.log("Received query:", query);
    let output = "";
    let command = `
    cd ../
    ./target/debug/tf_idf -q ${query}`;
    console.log("Cammand:", command);
    // Execute the command
    const childProcess = (0, child_process_1.exec)(command, (error, stdout, stderr) => {
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

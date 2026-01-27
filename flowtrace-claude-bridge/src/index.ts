import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import axios from 'axios';

const FLOWTRACE_URL = process.env.FLOWTRACE_URL || "http://127.0.0.1:9601/mcp";

async function main() {
    const transport = new StdioServerTransport();

    transport.onmessage = async (message) => {
        // console.error("Received message:", JSON.stringify(message)); // Debug to stderr

        try {
            // Forward to Flowtrace
            const response = await axios.post(FLOWTRACE_URL, message, {
                headers: {
                    'Content-Type': 'application/json'
                }
            });

            // Forward response back to Claude
            if (response.data) {
                // console.error("Sending response:", JSON.stringify(response.data));
                transport.send(response.data as any);
            }
        } catch (error: any) {
            console.error("Error forwarding to Flowtrace:", error.message);
            if (error.response) {
                console.error("Response data:", JSON.stringify(error.response.data));
                // If message was a request (has id), send error response
                if ((message as any).id) {
                    transport.send({
                        jsonrpc: "2.0",
                        id: (message as any).id,
                        error: {
                            code: -32603, // Internal error
                            message: `Flowtrace Error: ${error.message}`
                        }
                    } as any);
                }
            }
        }
    };

    await transport.start();
    console.error(`Flowtrace Bridge started, forwarding to ${FLOWTRACE_URL}`);
}

main().catch((err) => {
    console.error("Bridge failed:", err);
    process.exit(1);
});

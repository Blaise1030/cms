import { collectionRegistry } from "../src/content-schemas.mjs";

try {
  let input = "";
  for await (const chunk of process.stdin) input += chunk;
  const request = JSON.parse(input);
  if (request.op !== "list") throw new Error(`Unsupported operation: ${request.op}`);
  process.stdout.write(JSON.stringify({
    collections: Object.entries(collectionRegistry).map(([name, definition]) => ({ name, ...definition, supportedForEditing: ["site", "pages", "sections", "blog", "helpDocs"].includes(name) })),
  }) + "\n");
} catch (error) {
  process.stderr.write(`${error.message}\n`);
  process.exitCode = 1;
}

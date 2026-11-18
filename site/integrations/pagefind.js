import { fileURLToPath } from "node:url";
import * as pagefind from "pagefind";

/**
 * Builds a Pagefind search index per section after `astro build`, so the blog search only
 * returns posts and the help search only returns articles. Each bundle lands at
 * `<base>pagefind-<name>/` in the build output. Indexes only exist after a build; under
 * `astro dev` the search boxes load nothing.
 *
 * @param {{ sections: { name: string, glob: string }[] }} options
 * @returns {import("astro").AstroIntegration}
 */
export function pagefindIndex({ sections }) {
  return {
    name: "orso-pagefind",
    hooks: {
      "astro:build:done": async ({ dir, logger }) => {
        const outDir = fileURLToPath(dir);
        for (const section of sections) {
          const { index, errors } = await pagefind.createIndex();
          if (!index) throw new Error(`Pagefind could not start: ${errors.join(", ")}`);
          const added = await index.addDirectory({ path: outDir, glob: section.glob });
          if (added.errors.length) throw new Error(`Pagefind (${section.name}): ${added.errors.join(", ")}`);
          const written = await index.writeFiles({ outputPath: `${outDir}/pagefind-${section.name}` });
          if (written.errors.length) throw new Error(`Pagefind (${section.name}): ${written.errors.join(", ")}`);
          logger.info(`Indexed ${added.page_count} ${section.name} pages`);
        }
        await pagefind.close();
      },
    },
  };
}

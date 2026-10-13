import { defineCollection } from "astro:content";
import { glob } from "astro/loaders";
import { z } from "astro/zod";
import { collectionRegistry, createCollectionSchemas } from "./content-schemas.mjs";

// Explicit keys retain Astro's collection-name inference; every loader uses the registry.
const collection = (name: keyof typeof collectionRegistry) => defineCollection({
  loader: glob(collectionRegistry[name].loader),
  schema: (context) => createCollectionSchemas({ z, ...context })[name],
});
export const collections = {
  blog: collection("blog"),
  helpDocs: collection("helpDocs"),
};

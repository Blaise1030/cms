import type { InjectionKey } from "vue";

/** Entries a reference field can choose, by target collection; provided by the Content panel. */
export interface ReferenceEntries {
  /** Entry ids of `collection`, or undefined until `load` has fetched them. */
  entries(collection: string): string[] | undefined;
  load(collection: string): void;
}
export const REFERENCE_ENTRIES: InjectionKey<ReferenceEntries> = Symbol("content-reference-entries");

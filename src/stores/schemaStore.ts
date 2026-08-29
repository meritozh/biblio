import { Store, useStore } from '@tanstack/react-store';
import type { SchemaDefinition } from '@/types';
import { schemaList } from '@/lib/schemaBridge';

interface SchemaState {
  schemas: SchemaDefinition[];
  /** False until the first successful load. Readers fall back to a
   *  static copy of the built-in seeds (see `categorySchema.ts`) so
   *  pre-load renders behave exactly like the old hard-coded registry. */
  loaded: boolean;
}

const initialState: SchemaState = {
  schemas: [],
  loaded: false,
};

export const schemaStore = new Store<SchemaState>(initialState);

/** Fetch every schema definition from the DB. Called once at app boot
 *  (alongside `loadCategories`) and again after any schema mutation. */
export async function loadSchemas(): Promise<void> {
  const schemas = await schemaList();
  schemaStore.setState(() => ({ schemas, loaded: true }));
}

/** Reactive accessor for components that render schema options/labels
 *  and need to update when definitions change. */
export function useSchemas(): SchemaDefinition[] {
  return useStore(schemaStore, (s) => s.schemas);
}

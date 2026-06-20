import type { DynamicMetadataFormValues } from '@/components/DynamicMetadataForm';
import type {
  Author,
  Category,
  DuplicateAction,
  FileAnalysisStatus,
  FilePreparedImport,
  Tag,
} from '@/types';

export type FileStatus = FileAnalysisStatus;

/**
 * Which panel a file belongs in:
 *   - `processing`: still being analyzed — not yet in any tab, shown only as a
 *     header-level progress counter.
 *   - `review`: analysis complete but needs human attention (duplicate detected
 *     OR partial metadata extraction).
 *   - `ready`: analysis complete, all signals healthy, safe to batch-import.
 *   - `failed`: analysis errored. Cannot be imported.
 */
export type Bucket = 'processing' | 'review' | 'ready' | 'failed';

export interface FileItemState {
  path: string;
  fileName: string;
  status: FileStatus;
  selected: boolean;
  preparedImport?: FilePreparedImport;
  formValues: DynamicMetadataFormValues;
  error?: string;
  userEdited: boolean;
  suggestedTags: string[];
  /** LLM-extracted author names that didn't resolve against the existing
   *  catalog. Surface as chips — the user adopts (find-or-create on the
   *  authors snapshot) or dismisses. Authors are never auto-created. */
  suggestedAuthors: string[];
  duplicateAction: DuplicateAction | null;
}

export interface ProcessingPipelineProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  /** Minimize collapses the modal into a small floating pill while
   *  leaving every internal listener and the per-file state intact —
   *  the worker keeps analyzing in the background and the user can
   *  re-open later to commit. Mirrors the RemoteUploadProgressPanel
   *  minimize/expand convention so the two panels feel symmetric. */
  minimized: boolean;
  onMinimize: () => void;
  onExpand: () => void;
  paths: string[];
  /** Per-path map of source folder. Keys are entries in `paths`; values
   *  are the folders the user picked. Empty for non-folder picks. The
   *  set of unique values gives the list of roots to clean up after
   *  import; per-path values let the backend group comics by root for
   *  the parent-dir author hint. */
  pathFolderRoots?: Record<string, string>;
  /** Category-first import: the category the user is importing INTO. Drives
   *  the default category for every reviewed file (the backend already
   *  validated the input against this category's schema). Falls back to
   *  extension-based schema inference when null (legacy / no selection). */
  targetCategoryId?: number | null;
  categories: Category[];
  tags: Tag[];
  authors: Author[];
  onTagCreate: (name: string) => Promise<Tag>;
  onAuthorCreate: (name: string) => Promise<Author>;
  onImportComplete: () => void;
}

export const EMPTY_FORM_VALUES: DynamicMetadataFormValues = {
  display_name: '',
  category_id: null,
  tag_ids: [],
  author_ids: [],
  metadata: [],
  progress: '',
};

export type TabKey = 'review' | 'ready' | 'failed';

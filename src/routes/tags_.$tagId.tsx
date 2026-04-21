import { createFileRoute } from '@tanstack/react-router';
import { useEffect, useState, useCallback } from 'react';
import { Tag as TagIcon } from 'lucide-react';
import { FileDetailPage } from '@/components/FileDetailPage';
import { fileListByTag, tagList } from '@/lib/tauri';

export const Route = createFileRoute('/tags_/$tagId')({
  component: TagDetailPage,
});

function TagDetailPage() {
  const { tagId } = Route.useParams();
  const id = Number(tagId);
  const [tagName, setTagName] = useState<string>('');

  useEffect(() => {
    if (Number.isNaN(id)) return;
    void tagList(false).then((r) => {
      const found = r.tags.find((t) => t.id === id);
      setTagName(found?.name ?? `#${id}`);
    });
  }, [id]);

  const fetcher = useCallback(() => fileListByTag(id), [id]);

  if (Number.isNaN(id)) {
    return (
      <div className="flex h-screen items-center justify-center bg-background">
        <p className="text-sm text-muted-foreground font-serif-italic">
          Unknown tag reference.
        </p>
      </div>
    );
  }

  return (
    <FileDetailPage
      title={tagName || '…'}
      kind="Tag"
      icon={<TagIcon className="h-4 w-4" aria-hidden="true" />}
      backTo="/tags"
      fetcher={fetcher}
      filterKey={`tag:${id}`}
    />
  );
}

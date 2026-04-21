import { createFileRoute } from '@tanstack/react-router';
import { useEffect, useState, useCallback } from 'react';
import { User as UserIcon } from 'lucide-react';
import { FileDetailPage } from '@/components/FileDetailPage';
import { authorList, fileListByAuthor } from '@/lib/tauri';

export const Route = createFileRoute('/authors_/$authorId')({
  component: AuthorDetailPage,
});

function AuthorDetailPage() {
  const { authorId } = Route.useParams();
  const id = Number(authorId);
  const [authorName, setAuthorName] = useState<string>('');

  useEffect(() => {
    if (Number.isNaN(id)) return;
    void authorList(false).then((r) => {
      const found = r.authors.find((a) => a.id === id);
      setAuthorName(found?.name ?? `#${id}`);
    });
  }, [id]);

  const fetcher = useCallback(() => fileListByAuthor(id), [id]);

  if (Number.isNaN(id)) {
    return (
      <div className="flex h-screen items-center justify-center bg-background">
        <p className="text-sm text-muted-foreground font-serif-italic">
          Unknown author reference.
        </p>
      </div>
    );
  }

  return (
    <FileDetailPage
      title={authorName || '…'}
      kind="Author"
      icon={<UserIcon className="h-4 w-4" aria-hidden="true" />}
      backTo="/authors"
      fetcher={fetcher}
      filterKey={`author:${id}`}
    />
  );
}

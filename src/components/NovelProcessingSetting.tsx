import { useState, useEffect } from 'react';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import { settingsGet, settingsSet } from '@/lib/tauri';

const KEY_EPUB = 'process_novel_epub';
const KEY_PDF = 'process_novel_pdf';

function parseBool(value: string | null, fallback: boolean): boolean {
  if (value === null) return fallback;
  return value === '1' || value.toLowerCase() === 'true';
}

export function NovelProcessingSetting() {
  const [epub, setEpub] = useState(true);
  const [pdf, setPdf] = useState(false);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    Promise.all([settingsGet(KEY_EPUB), settingsGet(KEY_PDF)])
      .then(([e, p]) => {
        setEpub(parseBool(e, true));
        setPdf(parseBool(p, false));
      })
      .finally(() => setLoading(false));
  }, []);

  const handleEpub = async (checked: boolean) => {
    setEpub(checked);
    await settingsSet(KEY_EPUB, checked ? 'true' : 'false');
  };

  const handlePdf = async (checked: boolean) => {
    setPdf(checked);
    await settingsSet(KEY_PDF, checked ? 'true' : 'false');
  };

  if (loading) {
    return <div className="text-sm text-muted-foreground min-h-[60px]">Loading...</div>;
  }

  return (
    <div className="space-y-3">
      <div>
        <h3 className="text-sm font-semibold">Novel Processing</h3>
        <p className="text-xs text-muted-foreground">
          Choose which book formats go through the LLM novel pipeline (title, authors, tags, description).
          Plain-text (.txt) files are always processed.
        </p>
      </div>

      <div className="flex items-center justify-between">
        <div>
          <Label htmlFor="novel-process-epub" className="text-xs">
            Process EPUB files
          </Label>
          <p className="text-xs text-muted-foreground">
            Sample chapters from .epub archives and send them to the LLM
          </p>
        </div>
        <Switch
          id="novel-process-epub"
          checked={epub}
          onCheckedChange={handleEpub}
        />
      </div>

      <div className="flex items-center justify-between">
        <div>
          <Label htmlFor="novel-process-pdf" className="text-xs">
            Process PDF files
          </Label>
          <p className="text-xs text-muted-foreground">
            Extract text from the first pages of .pdf files for LLM analysis
          </p>
        </div>
        <Switch
          id="novel-process-pdf"
          checked={pdf}
          onCheckedChange={handlePdf}
        />
      </div>
    </div>
  );
}

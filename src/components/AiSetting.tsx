import { useState, useEffect } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import { Badge } from '@/components/ui/badge';
import { llmConfigGet, llmConfigSet, llmTestConnection } from '@/lib/tauri';
import type { LlmConfig } from '@/types';
import { Loader2, Eye, EyeOff, Check, AlertCircle } from 'lucide-react';

const defaultConfig: LlmConfig = {
  enabled: false,
  base_url: 'http://localhost:11434/v1',
  api_key: '',
  model: 'llama3.2',
  mode: 'metadata_only',
};

export function AiSetting() {
  const [config, setConfig] = useState<LlmConfig>(defaultConfig);
  const [loading, setLoading] = useState(true);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<{ ok: boolean; message: string } | null>(null);
  const [saving, setSaving] = useState(false);
  const [saved, setSaved] = useState(false);
  const [showApiKey, setShowApiKey] = useState(false);

  useEffect(() => {
    llmConfigGet()
      .then((c) => {
        setConfig(c);
        setLoading(false);
      })
      .catch(() => {
        setConfig(defaultConfig);
        setLoading(false);
      });
  }, []);

  const updateField = <K extends keyof LlmConfig>(key: K, value: LlmConfig[K]) => {
    setConfig((prev) => ({ ...prev, [key]: value }));
    setTestResult(null);
  };

  const handleTest = async () => {
    setTesting(true);
    setTestResult(null);
    try {
      const result = await llmTestConnection();
      setTestResult({ ok: true, message: result });
    } catch (err) {
      setTestResult({ ok: false, message: err instanceof Error ? err.message : String(err) });
    } finally {
      setTesting(false);
    }
  };

  const handleSave = async () => {
    setSaving(true);
    setSaved(false);
    setTestResult(null);
    try {
      await llmConfigSet(config);
      setSaved(true);
      setTimeout(() => setSaved(false), 3000);
    } catch (err) {
      setTestResult({ ok: false, message: err instanceof Error ? err.message : String(err) });
    } finally {
      setSaving(false);
    }
  };

  if (loading) {
    return <div className="text-sm text-muted-foreground min-h-[100px]">Loading...</div>;
  }

  const disabled = !config.enabled;

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h3 className="text-sm font-semibold">AI Assistant</h3>
          <p className="text-xs text-muted-foreground">
            Configure a local or remote LLM to enhance file metadata extraction
          </p>
        </div>
        <div className="flex items-center gap-2">
          <Label htmlFor="llm-enabled" className="text-xs">
            {config.enabled ? 'Enabled' : 'Disabled'}
          </Label>
          <Switch
            id="llm-enabled"
            checked={config.enabled}
            onCheckedChange={(checked) => updateField('enabled', checked)}
          />
        </div>
      </div>

      <div className="space-y-3">
        <div className="space-y-1.5">
          <Label htmlFor="llm-base-url" className="text-xs">
            Base URL
          </Label>
          <Input
            id="llm-base-url"
            value={config.base_url}
            onChange={(e) => updateField('base_url', e.target.value)}
            disabled={disabled}
            placeholder="http://localhost:11434/v1"
            className="text-sm"
          />
          <p className="text-xs text-muted-foreground">
            OpenAI Compatible Provider, e.g. http://localhost:11434/v1
          </p>
        </div>

        <div className="space-y-1.5">
          <Label htmlFor="llm-api-key" className="text-xs">
            API Key
          </Label>
          <div className="relative">
            <Input
              id="llm-api-key"
              type={showApiKey ? 'text' : 'password'}
              value={config.api_key}
              onChange={(e) => updateField('api_key', e.target.value)}
              disabled={disabled}
              placeholder="Optional for local LLMs"
              className="text-sm pr-10"
            />
            <button
              type="button"
              onClick={() => setShowApiKey(!showApiKey)}
              disabled={disabled}
              className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground disabled:opacity-50"
              aria-label={showApiKey ? 'Hide API key' : 'Show API key'}
            >
              {showApiKey ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
            </button>
          </div>
          <p className="text-xs text-muted-foreground">Optional, required for remote providers</p>
        </div>

        <div className="space-y-1.5">
          <Label htmlFor="llm-model" className="text-xs">
            Model Name
          </Label>
          <Input
            id="llm-model"
            value={config.model}
            onChange={(e) => updateField('model', e.target.value)}
            disabled={disabled}
            placeholder="llama3.2"
            className="text-sm"
          />
        </div>

        <div className="space-y-1.5">
          <Label className="text-xs">Mode</Label>
          <div className="flex gap-2">
            <button
              type="button"
              onClick={() => updateField('mode', 'metadata_only')}
              disabled={disabled}
              className={`flex items-center gap-2 px-3 py-2 rounded-xl text-xs font-medium transition-colors duration-200 ${
                config.mode === 'metadata_only'
                  ? 'bg-primary text-primary-foreground'
                  : 'bg-secondary text-secondary-foreground hover:bg-secondary/80'
              } ${disabled ? 'opacity-50 cursor-not-allowed' : ''}`}
              aria-pressed={config.mode === 'metadata_only'}
            >
              Metadata only
            </button>
            <button
              type="button"
              onClick={() => updateField('mode', 'with_content')}
              disabled={disabled}
              className={`flex items-center gap-2 px-3 py-2 rounded-xl text-xs font-medium transition-colors duration-200 ${
                config.mode === 'with_content'
                  ? 'bg-primary text-primary-foreground'
                  : 'bg-secondary text-secondary-foreground hover:bg-secondary/80'
              } ${disabled ? 'opacity-50 cursor-not-allowed' : ''}`}
              aria-pressed={config.mode === 'with_content'}
            >
              With content
            </button>
          </div>
          <p className="text-xs text-muted-foreground">
            {config.mode === 'metadata_only'
              ? 'Sends only filename + extracted metadata (fast, cheap)'
              : 'Also extracts text from PDFs (more accurate)'}
          </p>
        </div>

        <div className="flex items-center gap-3 pt-2">
          <Button variant="outline" size="sm" onClick={handleTest} disabled={disabled || testing}>
            {testing ? <Loader2 className="h-4 w-4 animate-spin" /> : 'Test Connection'}
          </Button>
          {testResult && (
            <div className="flex items-center gap-2">
              {testResult.ok ? (
                <Badge variant="green">
                  <Check className="h-3 w-3 mr-1" />
                  Connected
                </Badge>
              ) : (
                <span className="text-xs text-destructive flex items-center gap-1">
                  <AlertCircle className="h-3 w-3" />
                  {testResult.message}
                </span>
              )}
            </div>
          )}
        </div>

        <div className="flex items-center gap-3 pt-2">
          <Button size="sm" onClick={handleSave} disabled={saving}>
            {saving ? <Loader2 className="h-4 w-4 animate-spin" /> : 'Save'}
          </Button>
          {saved && (
            <Badge variant="green">
              <Check className="h-3 w-3 mr-1" />
              Saved
            </Badge>
          )}
        </div>
      </div>
    </div>
  );
}

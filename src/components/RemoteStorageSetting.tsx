import { useEffect, useState } from 'react';
import { AlertCircle, Check, ChevronRight, Eye, EyeOff, Loader2, LogOut, X } from 'lucide-react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { cn } from '@/lib/utils';
import { remoteConfigGet, remoteLogin, remoteLogout, remoteGetAuthorizeUrl } from '@/lib/tauri';
import { openUrl } from '@tauri-apps/plugin-opener';
import { parseTokenInput } from '@/lib/baidu_oauth';
import type { RemoteConfig } from '@/types';

const DEFAULT_APP_ROOT = '/apps/biblio';

export function RemoteStorageSetting() {
  const [config, setConfig] = useState<RemoteConfig | null>(null);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [success, setSuccess] = useState<string | null>(null);

  const [appKey, setAppKey] = useState('');
  const [tokenInput, setTokenInput] = useState('');
  const [appRoot, setAppRoot] = useState(DEFAULT_APP_ROOT);
  const [showToken, setShowToken] = useState(false);

  useEffect(() => {
    remoteConfigGet()
      .then((cfg) => {
        setConfig(cfg);
        setAppKey(cfg.app_key ?? '');
        setAppRoot(cfg.app_root || DEFAULT_APP_ROOT);
        setLoading(false);
      })
      .catch(() => setLoading(false));
  }, []);

  const handleOpenAuthUrl = async () => {
    if (!appKey.trim()) {
      setError('Enter your AppKey first.');
      return;
    }
    try {
      const url = await remoteGetAuthorizeUrl(appKey.trim());
      await openUrl(url);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  };

  const handleLogin = async () => {
    setBusy(true);
    setError(null);
    setSuccess(null);

    const parsed = parseTokenInput(tokenInput);
    if (!parsed) {
      setError('Please paste the access token or redirect URL.');
      setBusy(false);
      return;
    }
    if (!appKey.trim()) {
      setError('AppKey is required.');
      setBusy(false);
      return;
    }

    try {
      const updated = await remoteLogin({
        app_key: appKey.trim(),
        access_token: parsed.access_token,
        expires_in_secs: parsed.expires_in_secs,
        app_root: appRoot.trim() || null,
      });
      setConfig(updated);
      setTokenInput('');
      setSuccess('Connected — access token stored.');
      setTimeout(() => setSuccess(null), 3000);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleLogout = async () => {
    setBusy(true);
    setError(null);
    try {
      await remoteLogout();
      const fresh = await remoteConfigGet();
      setConfig(fresh);
      setSuccess('Signed out of Baidu Pan.');
      setTimeout(() => setSuccess(null), 3000);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  if (loading) {
    return <div className="text-sm text-muted-foreground min-h-[100px]">Loading...</div>;
  }

  const isConnected = config?.enabled ?? false;
  const expiresInSecs = config ? config.access_token_expires_at - Math.floor(Date.now() / 1000) : 0;
  const expiresHumanReadable =
    expiresInSecs > 0
      ? expiresInSecs > 86400
        ? `${Math.floor(expiresInSecs / 86400)}d ${Math.floor((expiresInSecs % 86400) / 3600)}h`
        : `${Math.floor(expiresInSecs / 3600)}h ${Math.floor((expiresInSecs % 3600) / 60)}m`
      : 'expired';

  return (
    <div className="space-y-4">
      <div>
        <h3 className="text-sm font-semibold">Remote Storage (Baidu Pan)</h3>
        <p className="text-xs text-muted-foreground">
          Upload comic archives to Baidu Netdisk. Biblio keeps metadata + cover locally;
          the archive file lives on Baidu under an obfuscated filename.
        </p>
      </div>

      {isConnected && config ? (
        <div className="space-y-3 rounded-xl border border-success/30 bg-success-muted dark:bg-success/10 p-3">
          <div className="flex items-center justify-between flex-wrap gap-2">
            <div className="flex items-center gap-2">
              <Badge variant="green">
                <Check className="h-3 w-3 mr-1" /> Connected
              </Badge>
            </div>
            <div className="flex items-center gap-2">
              {expiresInSecs > 0 && expiresInSecs < 604800 && (
                <span className="text-xs text-amber-500 flex items-center gap-1">
                  <AlertCircle className="h-3 w-3" /> Expires soon
                </span>
              )}
              {expiresInSecs <= 0 && (
                <span className="text-xs text-destructive">Token expired — please re-authenticate</span>
              )}
              <Button variant="outline" size="sm" onClick={handleLogout} disabled={busy}>
                <LogOut className="h-3 w-3 mr-1" /> Sign out
              </Button>
            </div>
          </div>
          <div className="text-xs text-muted-foreground space-y-0.5">
            <div>AppKey: <span className="font-mono">{config.app_key.slice(0, 8)}...</span></div>
            <div>Upload root: <span className="font-mono">{config.app_root}</span></div>
            <div>Access token: {expiresHumanReadable}</div>
          </div>
        </div>
      ) : (
        <div className="space-y-3">
          <CollapsibleHelp title="How to get your AppKey and Access Token">
            <ol className="text-xs text-muted-foreground space-y-1 list-decimal list-inside">
              <li>Create an app at{' '}
                <a href="https://pan.baidu.com/union/console" target="_blank" rel="noreferrer"
                   className="text-primary underline">pan.baidu.com/union/console</a>
              </li>
              <li>Copy the AppKey from your app's settings</li>
              <li>Enter the AppKey below and click "Open Authorization Page"</li>
              <li>Log into Baidu and authorize the app</li>
              <li>On the redirect page, copy the <code className="font-mono bg-secondary px-1 rounded">access_token</code> from the URL</li>
              <li>Paste it in the Access Token field below</li>
            </ol>
          </CollapsibleHelp>

          <div className="space-y-1.5">
            <Label htmlFor="baidu-app-key" className="text-xs">AppKey</Label>
            <Input
              id="baidu-app-key"
              value={appKey}
              onChange={(e) => setAppKey(e.target.value)}
              placeholder="Your Baidu AppKey from pan.baidu.com/union/console"
              className="text-sm"
            />
          </div>

          <Button variant="outline" size="sm" onClick={handleOpenAuthUrl} disabled={!appKey.trim()}>
            Open Authorization Page
          </Button>

          <div className="space-y-1.5">
            <Label htmlFor="baidu-token" className="text-xs">
              Access Token
            </Label>
            <div className="relative">
              <Input
                id="baidu-token"
                type={showToken ? 'text' : 'password'}
                value={tokenInput}
                onChange={(e) => setTokenInput(e.target.value)}
                placeholder="Paste the access_token or full redirect URL here"
                className="text-sm pr-10"
              />
              <button
                type="button"
                onClick={() => setShowToken(!showToken)}
                className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
                aria-label={showToken ? 'Hide token' : 'Show token'}
              >
                {showToken ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
              </button>
            </div>
            <p className="text-xs text-muted-foreground">
              After authorizing in the browser, copy the access token from the redirect page and paste it here.
            </p>
          </div>

          <div className="space-y-1.5">
            <Label htmlFor="baidu-app-root" className="text-xs">
              Upload directory
            </Label>
            <Input
              id="baidu-app-root"
              value={appRoot}
              onChange={(e) => setAppRoot(e.target.value)}
              placeholder="/apps/biblio"
              className="text-sm font-mono"
            />
            <p className="text-xs text-muted-foreground">
              Absolute path under your Baidu Pan. Must live inside an <code className="font-mono bg-secondary px-1 rounded">/apps/...</code> directory;
              biblio creates it implicitly on first upload.
            </p>
            {appRoot && !appRoot.startsWith('/apps/') && (
              <p className="text-xs text-destructive flex items-center gap-1">
                <AlertCircle className="h-3 w-3" />
                Baidu OpenAPI requires paths under /apps/ — uploads may fail
              </p>
            )}
          </div>

          <Button size="sm" onClick={handleLogin} disabled={busy || !tokenInput.trim() || !appKey.trim()}>
            {busy ? <Loader2 className="h-4 w-4 animate-spin mr-1" /> : null}
            Connect
          </Button>
        </div>
      )}

      {error && (
        <div className="flex items-start gap-2 text-xs text-destructive bg-destructive/5 rounded-lg p-2">
          <AlertCircle className="h-3 w-3 mt-0.5 shrink-0" />
          <span className="break-words flex-1">{error}</span>
          <button type="button" onClick={() => setError(null)} className="shrink-0 hover:text-destructive/80">
            <X className="h-3 w-3" />
          </button>
        </div>
      )}
      {success && (
        <div className="flex items-center gap-2 text-xs text-success">
          <Check className="h-3 w-3" />
          {success}
        </div>
      )}
    </div>
  );
}

function CollapsibleHelp({ title, children }: { title: string; children: React.ReactNode }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="rounded-lg border border-border">
      <button
        type="button"
        onClick={() => setOpen(!open)}
        className="flex items-center gap-1.5 w-full p-2 text-xs font-medium text-left hover:bg-secondary/50 transition-colors duration-200"
      >
        <ChevronRight className={cn('h-3 w-3 transition-transform duration-200', open && 'rotate-90')} />
        {title}
      </button>
      {open && <div className="px-2 pb-2">{children}</div>}
    </div>
  );
}

import { useCallback, useEffect, useState } from 'react';
import {
  AlertCircle,
  Check,
  ChevronRight,
  Copy,
  Eye,
  EyeOff,
  KeyRound,
  Loader2,
  Lock,
  LogOut,
  ShieldCheck,
  X,
} from 'lucide-react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { cn } from '@/lib/utils';
import {
  remoteConfigGet,
  remoteLogin,
  remoteLogout,
  remoteGetAuthorizeUrl,
  remoteLegacyCount,
  reencryptLegacy,
  remoteRecoveryKey,
  onRemoteReencryptProgress,
} from '@/lib/tauri';
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
          Upload comic archives to Baidu Netdisk. Biblio keeps metadata + cover locally and
          encrypts each archive on this device before upload — Baidu only ever stores opaque,
          unreadable bytes under a random filename.
        </p>
      </div>

      {isConnected && config ? (
        <>
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
        <EncryptionTools />
        </>
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

/** Recovery-key backup and "encrypt existing cloud files" backfill. Rendered
 *  only while connected. Self-contained: owns its own count/progress/key
 *  state and a long-lived re-encrypt progress listener. */
function EncryptionTools() {
  const [legacyCount, setLegacyCount] = useState<number | null>(null);
  const [progress, setProgress] = useState<{ done: number; total: number; failed: number } | null>(
    null
  );
  const [running, setRunning] = useState(false);
  const [recoveryKey, setRecoveryKey] = useState<string | null>(null);
  const [showKey, setShowKey] = useState(false);
  const [copied, setCopied] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  const refreshCount = useCallback(() => {
    remoteLegacyCount()
      .then(setLegacyCount)
      .catch(() => setLegacyCount(null));
  }, []);

  useEffect(() => {
    refreshCount();
  }, [refreshCount]);

  // One long-lived listener: the worker emits per-file events, we just count
  // terminal states against the total we queued.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;
    onRemoteReencryptProgress((p) => {
      if (p.status === 'success' || p.status === 'error' || p.status === 'skipped') {
        setProgress((prev) =>
          prev
            ? {
                ...prev,
                done: prev.done + 1,
                failed: prev.failed + (p.status === 'error' ? 1 : 0),
              }
            : prev
        );
      }
    }).then((u) => {
      if (cancelled) u();
      else unlisten = u;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  // When every queued file has reached a terminal state, stop and refresh the
  // remaining-raw count from the DB.
  useEffect(() => {
    if (running && progress && progress.done >= progress.total) {
      setRunning(false);
      refreshCount();
    }
  }, [running, progress, refreshCount]);

  const handleEncryptAll = async () => {
    setErr(null);
    try {
      const total = await reencryptLegacy();
      if (total === 0) {
        refreshCount();
        return;
      }
      setProgress({ done: 0, total, failed: 0 });
      setRunning(true);
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    }
  };

  const handleToggleKey = async () => {
    setErr(null);
    try {
      if (!recoveryKey) setRecoveryKey(await remoteRecoveryKey());
      setShowKey((s) => !s);
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    }
  };

  const handleCopyKey = async () => {
    setErr(null);
    try {
      const key = recoveryKey ?? (await remoteRecoveryKey());
      setRecoveryKey(key);
      await navigator.clipboard.writeText(key);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    }
  };

  return (
    <div className="space-y-3 rounded-xl border border-border p-3">
      <div className="flex items-center gap-2">
        <Lock className="h-3.5 w-3.5 text-primary" />
        <h4 className="text-xs font-semibold">Encryption</h4>
      </div>

      {/* Recovery key backup */}
      <div className="space-y-1.5">
        <div className="flex items-center gap-2">
          <Button variant="outline" size="sm" onClick={handleToggleKey}>
            <KeyRound className="h-3 w-3 mr-1" />
            {showKey ? 'Hide recovery key' : 'Show recovery key'}
          </Button>
          {recoveryKey && (
            <Button variant="ghost" size="sm" onClick={handleCopyKey}>
              {copied ? <Check className="h-3 w-3 mr-1" /> : <Copy className="h-3 w-3 mr-1" />}
              {copied ? 'Copied' : 'Copy'}
            </Button>
          )}
        </div>
        {showKey && recoveryKey && (
          <code className="block break-all rounded-lg bg-secondary px-2 py-1.5 font-mono text-[11px]">
            {recoveryKey}
          </code>
        )}
        <p className="flex items-start gap-1 text-xs text-amber-600 dark:text-amber-500">
          <AlertCircle className="mt-0.5 h-3 w-3 shrink-0" />
          <span>
            Copy this key and back it up somewhere safe and offline. Without it, encrypted cloud
            files cannot be recovered if this device's database is lost.
          </span>
        </p>
      </div>

      {/* Legacy backfill */}
      <div className="border-t border-border pt-3">
        {legacyCount === null ? null : legacyCount === 0 && !running ? (
          <p className="flex items-center gap-1.5 text-xs text-success">
            <ShieldCheck className="h-3.5 w-3.5" />
            All cloud files are encrypted.
          </p>
        ) : (
          <div className="space-y-2">
            <p className="text-xs text-muted-foreground">
              {running && progress
                ? `Encrypting ${progress.done} / ${progress.total}${
                    progress.failed > 0 ? ` (${progress.failed} failed)` : ''
                  }…`
                : `${legacyCount} file${legacyCount === 1 ? '' : 's'} uploaded before encryption ${
                    legacyCount === 1 ? 'is' : 'are'
                  } still stored raw on Baidu.`}
            </p>
            <Button size="sm" onClick={handleEncryptAll} disabled={running}>
              {running ? (
                <Loader2 className="h-4 w-4 animate-spin mr-1" />
              ) : (
                <Lock className="h-3 w-3 mr-1" />
              )}
              {running ? 'Encrypting…' : 'Encrypt existing files'}
            </Button>
          </div>
        )}
      </div>

      {err && (
        <div className="flex items-start gap-2 rounded-lg bg-destructive/5 p-2 text-xs text-destructive">
          <AlertCircle className="mt-0.5 h-3 w-3 shrink-0" />
          <span className="flex-1 break-words">{err}</span>
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
